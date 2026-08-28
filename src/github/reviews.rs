//! Pull request review submission, with every inline comment validated against
//! the diff before it is sent.
//!
//! # Why
//!
//! GitHub rejects an ENTIRE review with `422 Unprocessable Entity` when a
//! single inline comment names a line that is not inside a diff hunk. Line
//! numbers come from the model, so one hallucinated line used to cost every
//! other finding in the same review -- observed as 10 occurrences of
//! `gh: Unprocessable Entity (HTTP 422)` in the daemon log.
//!
//! Comments are therefore partitioned against the diff before submission:
//! addressable ones are submitted inline, the rest are dropped, and every drop
//! emits a log line naming path, line, side and reason. A dropped finding is
//! still published in the review body, so validation removes the 422 without
//! removing the finding.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use std::collections::HashMap;
use tokio::process::Command;
use tracing::{info, warn};

use crate::exec::{ExecClass, run_bounded};
use crate::publish::{self, AnvilAction};
use crate::reviewer::{InlineReviewComment, ReviewResponse};

#[derive(Serialize)]
struct CreateReviewRequest {
    commit_id: String,
    body: String,
    event: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    comments: Vec<ReviewCommentPayload>,
}

/// One inline comment as GitHub receives it.
///
/// `side` is serialized unconditionally: GitHub defaults an omitted `side` to
/// `RIGHT`, so a validated LEFT comment on a deleted line would 422 the whole
/// review one layer below the validator.
#[derive(Serialize)]
struct ReviewCommentPayload {
    path: String,
    line: u64,
    side: String,
    body: String,
}

/// Submits a review whose inline comments have NOT been checked against a diff.
///
/// BLOCKED: the three call sites that reach this function live outside this
/// lane's file (`src/github/mod.rs`, and through it `src/merge_enlister.rs` and
/// `src/webhook/pipelines/review.rs`), and none of them threads
/// `PrDiffContext::diff_content` through yet. Without diff text nothing is
/// provably addressable, so no inline comment can be anchored: the empty diff
/// makes that explicit rather than letting an un-threaded caller bypass
/// validation silently (I22 -- a bypass by omission is not a mechanism).
/// Findings are preserved in the review body, and the drop log names each one.
pub async fn submit_pr_review_impl(
    repo: &str,
    pr_number: u64,
    head_sha: &str,
    review: &ReviewResponse,
) -> Result<()> {
    submit_pr_review_with_diff(repo, pr_number, head_sha, review, "").await
}

/// Submits a review, keeping only the inline comments the `diff` proves are
/// addressable.
pub async fn submit_pr_review_with_diff(
    repo: &str,
    pr_number: u64,
    head_sha: &str,
    review: &ReviewResponse,
    diff: &str,
) -> Result<()> {
    let validation = validate_comments_against_diff(diff, &review.comments);

    for line in validation.drop_log() {
        warn!("{}#{}: {}", repo, pr_number, line);
    }

    info!(
        "Submitting PR review for {}#{} with verdict: {} ({} proposed inline comments, {} addressable, {} dropped)",
        repo,
        pr_number,
        review.verdict,
        review.comments.len(),
        validation.kept.len(),
        validation.dropped.len()
    );

    let request = build_review_request(head_sha, review, &validation);

    let json_body = serde_json::to_string(&request)?;
    let endpoint = format!("repos/{}/pulls/{}/reviews", repo, pr_number);

    // `gh api --input -` needs a piped stdin, but the bounded runner drives the
    // child through `Command`'s own output collection, which closes stdin.
    // Handing `gh` the same JSON through a temp file sends a byte-identical
    // request body while keeping the call under a timeout with `kill_on_drop`.
    let mut body_file = tempfile::NamedTempFile::new()
        .context("Failed to create a temp file for the gh api request body")?;
    {
        use std::io::Write;
        body_file
            .write_all(json_body.as_bytes())
            .context("Failed to write the gh api request body")?;
        body_file
            .flush()
            .context("Failed to flush the gh api request body")?;
    }
    let body_path = body_file.path().to_string_lossy().into_owned();

    let mut cmd = Command::new("gh");
    cmd.args(["api", "--method", "POST", &endpoint, "--input", &body_path]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let output = run_bounded(cmd, ExecClass::Api, "gh api POST pull request review").await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "Submitting the review with {} validated inline comment(s) failed: {}. Retrying as a single summary comment.",
            validation.kept.len(),
            stderr
        );

        return submit_fallback_review(repo, pr_number, head_sha, review, &validation).await;
    }

    info!(
        "Successfully published PR review for {}#{}",
        repo, pr_number
    );
    Ok(())
}

async fn submit_fallback_review(
    repo: &str,
    pr_number: u64,
    head_sha: &str,
    review: &ReviewResponse,
    validation: &CommentValidation,
) -> Result<()> {
    let full_body = build_fallback_body(review, validation, head_sha);

    let endpoint = format!("repos/{}/issues/{}/comments", repo, pr_number);
    let mut cmd = Command::new("gh");
    cmd.args([
        "api",
        "--method",
        "POST",
        &endpoint,
        "-f",
        &format!("body={}", full_body),
    ]);

    let output = run_bounded(cmd, ExecClass::Api, "gh api POST fallback review comment").await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Fallback review comment failed: {}", stderr);
    }

    info!(
        "Successfully submitted PR review fallback for {}#{}",
        repo, pr_number
    );
    Ok(())
}

/// Why a model-proposed inline comment could not be submitted.
///
/// `DiffUnavailable` is kept distinct from `LineNotInDiffHunk` on purpose
/// (invariant I1): when the diff itself is missing or unparseable we have no
/// evidence about the line, so the log must say "no evidence" rather than
/// accuse the line of being outside the diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropReason {
    /// The comment's file does not appear in the diff at all.
    PathNotInDiff,
    /// The file is in the diff, but the line is not inside any hunk on that side.
    LineNotInDiffHunk,
    /// The diff text was empty or could not be parsed. Absent evidence.
    DiffUnavailable,
}

impl DropReason {
    pub const fn label(self) -> &'static str {
        match self {
            DropReason::PathNotInDiff => "path not in diff",
            DropReason::LineNotInDiffHunk => "line not in any diff hunk",
            DropReason::DiffUnavailable => "diff unavailable or unparseable",
        }
    }
}

/// One discarded finding, retained so it can be logged rather than vanish.
#[derive(Debug, Clone)]
pub struct DroppedComment {
    pub path: String,
    pub line: u64,
    pub side: String,
    pub reason: DropReason,
    /// The finding itself, carried so a dropped comment can still be published
    /// in the review body instead of disappearing with its line number.
    pub body: String,
}

/// The partition of model-proposed comments into submittable and dropped.
///
/// `kept.len() + dropped.len()` must always equal the input length: a finding
/// that appears in neither has been silently discarded.
#[derive(Debug, Clone, Default)]
pub struct CommentValidation {
    pub kept: Vec<InlineReviewComment>,
    pub dropped: Vec<DroppedComment>,
}

impl CommentValidation {
    /// One log line per dropped finding: what was dropped and why.
    pub fn drop_log(&self) -> Vec<String> {
        self.dropped
            .iter()
            .map(|d| {
                format!(
                    "dropped inline review comment {}:{} [{}] -- {}",
                    d.path,
                    d.line,
                    d.side,
                    d.reason.label()
                )
            })
            .collect()
    }
}

/// GitHub's own default when `side` is omitted, and the value used for any
/// side the model did not spell as `LEFT`.
fn normalize_side(side: &str) -> String {
    if side.trim().eq_ignore_ascii_case("LEFT") {
        "LEFT".to_string()
    } else {
        "RIGHT".to_string()
    }
}

/// Line ranges a single file contributes to one side of the diff, held as
/// `(start, count)` pairs. Deliberately NOT an inclusive end: a zero-count
/// hunk such as `@@ -0,0 +1,2 @@` underflows `start + count - 1` in `u64` and
/// yields a range that covers every line.
#[derive(Debug, Default, Clone)]
struct FileHunks {
    old: Vec<(u64, u64)>,
    new: Vec<(u64, u64)>,
}

impl FileHunks {
    fn covers(ranges: &[(u64, u64)], line: u64) -> bool {
        if line == 0 {
            return false;
        }
        ranges
            .iter()
            .any(|&(start, count)| count > 0 && line >= start && line - start < count)
    }
}

/// Parses `@@ -old_start[,old_count] +new_start[,new_count] @@` .
///
/// Returns `None` for any header that does not parse, which the caller treats
/// as absent evidence for the whole diff rather than as a verdict on a line.
fn parse_hunk_header(line: &str) -> Option<((u64, u64), (u64, u64))> {
    let rest = line.strip_prefix("@@ ")?;
    let (ranges, _) = rest.split_once(" @@")?;
    let mut parts = ranges.split_whitespace();
    let old = parts.next()?.strip_prefix('-')?;
    let new = parts.next()?.strip_prefix('+')?;
    if parts.next().is_some() {
        return None;
    }
    Some((parse_range(old)?, parse_range(new)?))
}

/// `12,3` -> `(12, 3)`; `12` -> `(12, 1)` (an omitted count means exactly one).
fn parse_range(spec: &str) -> Option<(u64, u64)> {
    match spec.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((spec.parse().ok()?, 1)),
    }
}

/// `--- a/src/alpha.rs` -> `Some("src/alpha.rs")`; `/dev/null` -> `None`.
fn parse_file_path(spec: &str) -> Option<String> {
    let path = spec.split('\t').next()?.trim_end();
    if path == "/dev/null" || path.is_empty() {
        return None;
    }
    let path = path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path);
    Some(path.to_string())
}

/// Indexes a unified diff by every path each file is known by, so a comment on
/// a renamed or deleted file resolves on either side.
///
/// Returns `None` when the text carries no parseable hunk at all, or when any
/// hunk header is malformed: partial parse of a corrupt diff would let a real
/// line be reported as "not in any hunk", which is a fabricated accusation.
fn parse_unified_diff(diff: &str) -> Option<HashMap<String, FileHunks>> {
    let mut files: HashMap<String, FileHunks> = HashMap::new();
    let mut names: Vec<String> = Vec::new();
    let mut hunks_seen = 0usize;

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            names.clear();
        } else if let Some(spec) = line.strip_prefix("--- ") {
            if let Some(p) = parse_file_path(spec)
                && !names.contains(&p)
            {
                names.push(p);
            }
        } else if let Some(spec) = line.strip_prefix("+++ ") {
            if let Some(p) = parse_file_path(spec)
                && !names.contains(&p)
            {
                names.push(p);
            }
        } else if line.starts_with("@@") {
            let (old, new) = parse_hunk_header(line)?;
            if names.is_empty() {
                return None;
            }
            hunks_seen += 1;
            for name in &names {
                let entry = files.entry(name.clone()).or_default();
                entry.old.push(old);
                entry.new.push(new);
            }
        }
    }

    if hunks_seen == 0 {
        return None;
    }
    Some(files)
}

/// Partitions model-proposed comments by whether `(path, line, side)` is
/// actually addressable in the PR diff.
pub fn validate_comments_against_diff(
    diff: &str,
    comments: &[InlineReviewComment],
) -> CommentValidation {
    let drop = |c: &InlineReviewComment, reason: DropReason| DroppedComment {
        path: c.path.clone(),
        line: c.line,
        side: normalize_side(&c.side),
        reason,
        body: c.body.clone(),
    };

    let files = match parse_unified_diff(diff) {
        Some(files) => files,
        None => {
            return CommentValidation {
                kept: Vec::new(),
                dropped: comments
                    .iter()
                    .map(|c| drop(c, DropReason::DiffUnavailable))
                    .collect(),
            };
        }
    };

    let mut validation = CommentValidation::default();
    for c in comments {
        let Some(hunks) = files.get(&c.path) else {
            validation.dropped.push(drop(c, DropReason::PathNotInDiff));
            continue;
        };
        let ranges = if normalize_side(&c.side) == "LEFT" {
            &hunks.old
        } else {
            &hunks.new
        };
        if FileHunks::covers(ranges, c.line) {
            validation.kept.push(c.clone());
        } else {
            validation
                .dropped
                .push(drop(c, DropReason::LineNotInDiffHunk));
        }
    }
    validation
}

/// Applies the mandatory signature exactly once, anchored to the revision.
///
/// The sha is threaded rather than defaulted: a review comment with no
/// revision anchor cannot be told apart from one describing a head that a
/// force-push has replaced, and the reader has no way to know which they are
/// looking at.
fn published(content: &str, head_sha: &str) -> String {
    if publish::is_signed(content) {
        content.trim_end().to_string()
    } else {
        publish::body(
            AnvilAction::Reviewed,
            content,
            publish::Judged::Rev(head_sha.to_string()),
        )
    }
}

/// Renders the findings that could not be anchored to a diff hunk, so a
/// dropped comment is still published rather than lost with its line number.
fn render_dropped(validation: &CommentValidation) -> String {
    let mut s = String::new();
    if validation.dropped.is_empty() {
        return s;
    }
    s.push_str("\n\n### Findings not addressable in the diff\n");
    for d in &validation.dropped {
        s.push_str(&format!(
            "- `{}:{}` [{}] -- {}: {}\n",
            d.path,
            d.line,
            d.side,
            d.reason.label(),
            d.body.trim()
        ));
    }
    s
}

/// Assembles the review request from the validated comment set.
///
/// Only `validation.kept` reaches the API; the dropped findings move into the
/// summary body, which is where "still submit the summary review when every
/// comment is dropped" is enforced.
fn build_review_request(
    head_sha: &str,
    review: &ReviewResponse,
    validation: &CommentValidation,
) -> CreateReviewRequest {
    let mut content = review.summary.trim_end().to_string();
    content.push_str(&render_dropped(validation));

    CreateReviewRequest {
        commit_id: head_sha.to_string(),
        body: published(&content, head_sha),
        event: review.verdict.clone(),
        comments: validation
            .kept
            .iter()
            .map(|c| ReviewCommentPayload {
                path: c.path.clone(),
                line: c.line,
                side: normalize_side(&c.side),
                body: published(&c.body, head_sha),
            })
            .collect(),
    }
}

/// The single comment posted when the review endpoint rejects the submission.
///
/// Enumerates the validated findings alongside the dropped ones -- an
/// unvalidated republication would put back the line numbers validation just
/// removed -- and goes through `crate::publish`, so the fallback carries the
/// mandatory signature like every other published artifact.
fn build_fallback_body(
    review: &ReviewResponse,
    validation: &CommentValidation,
    head_sha: &str,
) -> String {
    let mut content = review.summary.trim_end().to_string();

    if !validation.kept.is_empty() {
        content.push_str("\n\n### Inline findings\n");
        for c in &validation.kept {
            content.push_str(&format!(
                "- `{}:{}` [{}]: {}\n",
                c.path,
                c.line,
                normalize_side(&c.side),
                c.body.trim()
            ));
        }
    }
    content.push_str(&render_dropped(validation));

    published(&content, head_sha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reviewer::InlineReviewComment;

    // Alpha: new-side hunk covers exactly lines 10..=13; old-side 1..=3, with
    // old line 2 deleted. Beta: a new file, new-side lines 1..=2.
    const DIFF: &str = r#"diff --git a/src/alpha.rs b/src/alpha.rs
index 1111111..2222222 100644
--- a/src/alpha.rs
+++ b/src/alpha.rs
@@ -1,3 +10,4 @@ fn alpha() {
 context_ten
-removed_old_two
+added_eleven
+added_twelve
 context_thirteen
diff --git a/src/beta.rs b/src/beta.rs
new file mode 100644
index 0000000..3333333
--- /dev/null
+++ b/src/beta.rs
@@ -0,0 +1,2 @@
+beta_one
+beta_two
"#;

    fn c(path: &str, line: u64, side: &str) -> InlineReviewComment {
        InlineReviewComment {
            path: path.to_string(),
            line,
            side: side.to_string(),
            body: format!("finding at {path}:{line}"),
        }
    }

    fn reasons(v: &CommentValidation) -> Vec<DropReason> {
        v.dropped.iter().map(|d| d.reason).collect()
    }

    // -- 1. red -> green ---------------------------------------------------

    #[test]
    fn one_bad_line_no_longer_costs_every_inline_comment() {
        // The production failure: 10x HTTP 422. GitHub rejects the ENTIRE
        // review if any single line is outside the diff, so the two good
        // findings were lost along with the bad one.
        let proposed = vec![
            c("src/alpha.rs", 11, "RIGHT"),
            c("src/alpha.rs", 900, "RIGHT"), // never in the diff
            c("src/beta.rs", 2, "RIGHT"),
        ];
        let v = validate_comments_against_diff(DIFF, &proposed);
        assert_eq!(
            v.kept.len(),
            2,
            "the two addressable findings must survive the one bad line"
        );
        assert_eq!(v.dropped.len(), 1, "exactly the bad line must be dropped");
        assert_eq!(v.dropped[0].line, 900);
        assert_eq!(v.dropped[0].reason, DropReason::LineNotInDiffHunk);
    }

    #[test]
    fn every_dropped_finding_produces_a_log_line_naming_path_line_and_reason() {
        // "Never silently discard a finding without a log line" enforced by
        // mechanism (I22), not by a comment telling the author to log.
        let proposed = vec![
            c("src/alpha.rs", 900, "RIGHT"),
            c("src/never_touched.rs", 1, "RIGHT"),
        ];
        let v = validate_comments_against_diff(DIFF, &proposed);
        let log = v.drop_log();
        assert_eq!(
            log.len(),
            v.dropped.len(),
            "one log line per dropped finding, no more, no fewer"
        );
        assert!(
            log.iter().any(|l| l.contains("src/alpha.rs")
                && l.contains("900")
                && l.contains(DropReason::LineNotInDiffHunk.label())),
            "drop log must name path, line and reason: {log:?}"
        );
        assert!(
            log.iter().any(|l| l.contains("src/never_touched.rs")
                && l.contains(DropReason::PathNotInDiff.label())),
            "drop log must distinguish an unknown path: {log:?}"
        );
    }

    #[test]
    fn no_finding_is_ever_silently_discarded() {
        // Accounting invariant across every fixture shape, including the
        // absent-evidence ones: kept + dropped accounts for every input.
        let proposed = vec![
            c("src/alpha.rs", 10, "RIGHT"),
            c("src/alpha.rs", 14, "RIGHT"),
            c("src/never_touched.rs", 7, "RIGHT"),
            c("src/beta.rs", 1, "RIGHT"),
        ];
        for diff in [DIFF, "", "gh: Not Found (HTTP 404)", "@@ corrupt"] {
            let v = validate_comments_against_diff(diff, &proposed);
            assert_eq!(
                v.kept.len() + v.dropped.len(),
                proposed.len(),
                "findings vanished for diff fixture {diff:?}"
            );
            // Accounting is not enough: a finding that is dropped but never
            // logged has still disappeared without trace, and the daemon
            // looks healthier than it is. This must hold for the
            // absent-evidence fixtures too, where EVERY finding is dropped
            // and an implementation that only logs LineNotInDiffHunk and
            // PathNotInDiff would go silent exactly when it matters most.
            assert_eq!(
                v.drop_log().len(),
                v.dropped.len(),
                "every drop must produce a log line, including absent-evidence drops, for {diff:?}"
            );
        }
    }

    // -- 2. FALSE-GREEN prevention ----------------------------------------
    // Fixtures the validator MUST reject, asserted to keep failing forever.
    // Without these, validate_comments_against_diff is indistinguishable from
    // the identity function -- which is exactly what ships today.

    #[test]
    fn false_green_a_line_outside_every_hunk_is_never_submitted() {
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 900, "RIGHT")]);
        assert!(
            v.kept.is_empty(),
            "Expected False Green prevention: a hallucinated line number must be DROPPED"
        );
        assert_eq!(reasons(&v), vec![DropReason::LineNotInDiffHunk]);
    }

    #[test]
    fn false_green_a_file_absent_from_the_diff_is_never_submitted() {
        let v = validate_comments_against_diff(DIFF, &[c("src/never_touched.rs", 1, "RIGHT")]);
        assert!(
            v.kept.is_empty(),
            "Expected False Green prevention: a file not in the diff must be DROPPED"
        );
        assert_eq!(reasons(&v), vec![DropReason::PathNotInDiff]);
    }

    #[test]
    fn false_green_a_deleted_line_is_not_addressable_on_the_right_side() {
        // old line 2 was deleted; it exists on LEFT only. Submitting it as
        // RIGHT is one of the real 422 shapes.
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 2, "RIGHT")]);
        assert!(
            v.kept.is_empty(),
            "Expected False Green prevention: an old-side line number sent as RIGHT must be DROPPED"
        );
        // The diff parsed fine, so the reason is the line, not missing
        // evidence. Reporting DiffUnavailable here would be a fabricated
        // excuse in the other direction (I1 cuts both ways).
        assert_eq!(reasons(&v), vec![DropReason::LineNotInDiffHunk]);
    }

    #[test]
    fn false_green_a_pure_deletion_hunk_offers_no_right_side_line() {
        let deletion_only = "diff --git a/src/gone.rs b/src/gone.rs\n\
--- a/src/gone.rs\n\
+++ b/src/gone.rs\n\
@@ -5,2 +4,0 @@\n\
-line_five\n\
-line_six\n";
        let v = validate_comments_against_diff(deletion_only, &[c("src/gone.rs", 4, "RIGHT")]);
        assert!(
            v.kept.is_empty(),
            "Expected False Green prevention: a hunk with new-count 0 has no commentable RIGHT line"
        );
        // `@@ -5,2 +4,0 @@` is a well-formed header. A parser that treats a
        // zero new-count as unparseable would pass the assertion above for
        // the wrong reason and then declare the whole diff unavailable.
        assert_eq!(reasons(&v), vec![DropReason::LineNotInDiffHunk]);
    }

    // -- 3. FALSE-RED prevention ------------------------------------------
    // Legitimate findings that MUST keep being submitted, so the check does
    // not block real review work and get bypassed.

    #[test]
    fn false_red_every_addressable_finding_survives_validation() {
        let proposed = vec![
            c("src/alpha.rs", 11, "RIGHT"),
            c("src/alpha.rs", 12, "RIGHT"),
            c("src/beta.rs", 1, "RIGHT"),
            c("src/beta.rs", 2, "RIGHT"),
        ];
        let v = validate_comments_against_diff(DIFF, &proposed);
        assert!(
            v.dropped.is_empty(),
            "Expected False Red prevention: addressable findings must PASS: {:?}",
            v.dropped
        );
        assert_eq!(v.kept.len(), proposed.len());
    }

    #[test]
    fn false_red_a_context_line_inside_a_hunk_is_addressable() {
        // GitHub accepts comments on unchanged context lines within a hunk.
        // Restricting to '+' lines only would drop legitimate findings.
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 13, "RIGHT")]);
        assert!(
            v.dropped.is_empty(),
            "Expected False Red prevention: a context line inside a hunk must PASS"
        );
        // `dropped.is_empty()` alone is also satisfied by a validator that
        // returns two empty vectors, which would discard the finding without
        // recording it. Name the surviving comment.
        assert_eq!(v.kept.len(), 1, "the finding must be KEPT, not voided");
    }

    #[test]
    fn false_red_a_left_side_comment_on_a_deleted_line_is_addressable() {
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 2, "LEFT")]);
        assert!(
            v.dropped.is_empty(),
            "Expected False Red prevention: a LEFT comment on a deleted line must PASS"
        );
        assert_eq!(v.kept.len(), 1, "the finding must be KEPT, not voided");
    }

    #[test]
    fn false_red_an_approve_with_no_comments_and_no_diff_is_still_a_signed_review() {
        // The merge-enlister call site submits APPROVE with zero comments and
        // has no diff in scope, so it can only pass "". That must be
        // harmless -- nothing to validate, nothing to drop -- rather than an
        // error or an empty-diff short circuit that swallows the review.
        let review = ReviewResponse {
            summary: "All gates green.".to_string(),
            verdict: "APPROVE".to_string(),
            comments: Vec::new(),
        };
        let validation = validate_comments_against_diff("", &review.comments);
        assert!(validation.kept.is_empty());
        assert!(
            validation.dropped.is_empty(),
            "zero comments means zero drops even with no diff: {:?}",
            validation.dropped
        );
        let req = build_review_request("deadbeef", &review, &validation);
        assert!(req.comments.is_empty());
        assert_eq!(req.event, "APPROVE", "the verdict must survive intact");
        assert!(
            crate::publish::is_signed(&req.body),
            "published output must carry the mandatory signature"
        );
    }

    // -- 4. absent evidence ------------------------------------------------
    // No diff means no evidence. Dropping is correct; inventing the reason is
    // not (I1: never a fabricated accusation).

    #[test]
    fn absent_evidence_an_empty_diff_yields_diff_unavailable_not_an_accusation() {
        let v = validate_comments_against_diff("", &[c("src/alpha.rs", 11, "RIGHT")]);
        assert!(
            v.kept.is_empty(),
            "no diff means nothing is proven addressable"
        );
        assert_eq!(
            reasons(&v),
            vec![DropReason::DiffUnavailable],
            "absent evidence must not be reported as 'line not in diff'"
        );
    }

    #[test]
    fn absent_evidence_an_unparseable_diff_yields_diff_unavailable() {
        // What PrDiffContext actually carries when the fetch failed.
        let v = validate_comments_against_diff(
            "gh: Not Found (HTTP 404)",
            &[c("src/alpha.rs", 11, "RIGHT")],
        );
        assert_eq!(reasons(&v), vec![DropReason::DiffUnavailable]);
    }

    #[test]
    fn absent_evidence_a_corrupt_hunk_header_yields_diff_unavailable() {
        let corrupt = "diff --git a/src/alpha.rs b/src/alpha.rs\n\
--- a/src/alpha.rs\n\
+++ b/src/alpha.rs\n\
@@ -1,3 +1O,4 @@\n\
+added\n";
        let v = validate_comments_against_diff(corrupt, &[c("src/alpha.rs", 10, "RIGHT")]);
        assert_eq!(
            reasons(&v),
            vec![DropReason::DiffUnavailable],
            "a hunk header that does not parse is absent evidence, not a bad line"
        );
    }

    #[test]
    fn absent_evidence_the_summary_review_is_still_submitted_when_all_comments_drop() {
        let review = ReviewResponse {
            summary: "No addressable findings.".to_string(),
            verdict: "COMMENT".to_string(),
            comments: vec![c("src/alpha.rs", 900, "RIGHT")],
        };
        let validation = validate_comments_against_diff(DIFF, &review.comments);
        let req = build_review_request("deadbeef", &review, &validation);
        assert!(
            req.comments.is_empty(),
            "a dropped comment must not reach the API"
        );
        assert!(
            req.body.contains("No addressable findings."),
            "the summary review must still be submitted"
        );
        assert!(
            crate::publish::is_signed(&req.body),
            "published output must carry the mandatory signature"
        );
    }

    #[test]
    fn only_validated_comments_reach_the_request_payload() {
        let review = ReviewResponse {
            summary: "two findings".to_string(),
            verdict: "REQUEST_CHANGES".to_string(),
            comments: vec![
                c("src/alpha.rs", 11, "RIGHT"),
                c("src/alpha.rs", 900, "RIGHT"),
            ],
        };
        let validation = validate_comments_against_diff(DIFF, &review.comments);
        let req = build_review_request("deadbeef", &review, &validation);
        assert_eq!(
            req.comments.len(),
            1,
            "only the addressable finding is sent"
        );
        assert_eq!(req.comments[0].line, 11);
        assert!(
            crate::publish::is_signed(&req.comments[0].body),
            "each inline comment keeps the mandatory signature"
        );
    }

    #[test]
    fn false_green_a_kept_left_comment_is_never_submitted_as_a_right_comment() {
        // The hole the rest of this file leaves open. `false_red_a_left_side_
        // comment_on_a_deleted_line_is_addressable` REQUIRES alpha.rs:2 LEFT
        // to survive validation, and the payload has no `side` field at all.
        // GitHub defaults an omitted `side` to RIGHT; line 2 is not on the
        // new side; the ENTIRE review 422s -- the exact failure this lane
        // exists to remove, reintroduced one layer below the validator. An
        // implementation could satisfy every other test here and still emit
        // the production defect.
        //
        // Asserted on the SERIALIZED form, not on a struct field, so a field
        // that is added but never reaches the wire cannot satisfy it.
        let review = ReviewResponse {
            summary: "a finding on a deleted line".to_string(),
            verdict: "COMMENT".to_string(),
            comments: vec![c("src/alpha.rs", 2, "LEFT")],
        };
        let validation = validate_comments_against_diff(DIFF, &review.comments);
        assert_eq!(
            validation.kept.len(),
            1,
            "fixture precondition: the LEFT line is addressable"
        );
        let req = build_review_request("deadbeef", &review, &validation);
        let wire = serde_json::to_value(&req).expect("the request must serialize");
        assert_eq!(
            wire["comments"][0]["side"],
            serde_json::json!("LEFT"),
            "Expected False Green prevention: side must reach the wire rather than defaulting to RIGHT: {wire}"
        );
        assert_eq!(wire["comments"][0]["line"], serde_json::json!(2));
    }

    // -- 5. boundaries -----------------------------------------------------
    // Alpha's new-side hunk is exactly 10..=13.

    #[test]
    fn boundary_one_below_the_first_hunk_line_is_dropped() {
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 9, "RIGHT")]);
        assert!(v.kept.is_empty(), "line 9 is one below the hunk start");
        assert_eq!(reasons(&v), vec![DropReason::LineNotInDiffHunk]);
    }

    #[test]
    fn boundary_exactly_the_first_hunk_line_is_kept() {
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 10, "RIGHT")]);
        assert!(v.dropped.is_empty(), "line 10 is exactly the hunk start");
        assert_eq!(
            v.kept.len(),
            1,
            "the threshold line must be KEPT, not voided"
        );
    }

    #[test]
    fn boundary_exactly_the_last_hunk_line_is_kept() {
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 13, "RIGHT")]);
        assert!(v.dropped.is_empty(), "line 13 is exactly the hunk end");
        assert_eq!(
            v.kept.len(),
            1,
            "the threshold line must be KEPT, not voided"
        );
    }

    #[test]
    fn boundary_one_above_the_last_hunk_line_is_dropped() {
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 14, "RIGHT")]);
        assert!(v.kept.is_empty(), "line 14 is one above the hunk end");
        assert_eq!(reasons(&v), vec![DropReason::LineNotInDiffHunk]);
    }

    #[test]
    fn boundary_line_zero_is_never_addressable() {
        let v = validate_comments_against_diff(DIFF, &[c("src/alpha.rs", 0, "RIGHT")]);
        assert!(v.kept.is_empty(), "GitHub has no line 0");
        assert_eq!(reasons(&v), vec![DropReason::LineNotInDiffHunk]);

        // The case above proves little on its own: alpha's new side is
        // 10..=13, so line 0 is outside it by a wide margin and any range
        // check rejects it. The arithmetic hazard the name claims lives on
        // beta's `@@ -0,0 +1,2 @@`, whose OLD side is start 0 count 0 -- an
        // inclusive-end computed as `start + count - 1` underflows u64 there
        // and yields a range covering everything.
        let zero_count = validate_comments_against_diff(DIFF, &[c("src/beta.rs", 0, "LEFT")]);
        assert!(
            zero_count.kept.is_empty(),
            "a zero-count old side covers no line at all, least of all line 0"
        );
        assert_eq!(reasons(&zero_count), vec![DropReason::LineNotInDiffHunk]);
    }

    #[test]
    fn boundary_a_hunk_header_with_an_implicit_count_of_one_covers_exactly_one_line() {
        let single = "diff --git a/src/one.rs b/src/one.rs\n\
--- /dev/null\n\
+++ b/src/one.rs\n\
@@ -0,0 +1 @@\n\
+only_line\n";
        let inside = validate_comments_against_diff(single, &[c("src/one.rs", 1, "RIGHT")]);
        assert!(inside.dropped.is_empty(), "line 1 is the only covered line");
        assert_eq!(
            inside.kept.len(),
            1,
            "the covered line must be KEPT, not voided"
        );
        let outside = validate_comments_against_diff(single, &[c("src/one.rs", 2, "RIGHT")]);
        assert!(
            outside.kept.is_empty(),
            "line 2 is one above the single line"
        );
        assert_eq!(reasons(&outside), vec![DropReason::LineNotInDiffHunk]);
    }

    // -- 6. mechanism (I5, I22) -------------------------------------------

    /// The diff is already on `PrDiffContext`; stage 2 must thread it in, not
    /// shell out for it. A `gh pr diff` added here without `run_bounded`
    /// would reintroduce the unbounded child that I5 exists to prevent.
    ///
    /// Scans for the direct execution methods rather than counting spawns: a
    /// `Command` cannot run without `output`, `spawn` or `status`, so their
    /// absence proves every subprocess is handed to `crate::exec`. Enforced
    /// by mechanism, not by a comment asking the author to remember (I22).
    #[test]
    fn every_subprocess_in_this_file_is_bounded_by_crate_exec() {
        let src = include_str!("reviews.rs");
        let module_body = src
            .split("mod tests")
            .next()
            .expect("the non-test module body");
        for direct in [".output()", ".spawn()", ".status()"] {
            assert!(
                !module_body.contains(direct),
                "invariant I5: `{direct}` executes a child outside \
                 crate::exec::run_bounded, losing the timeout and kill_on_drop"
            );
        }
        assert!(
            module_body.contains("run_bounded("),
            "invariant I5: subprocess execution must go through crate::exec"
        );
    }

    // -- 7. fallback publication ------------------------------------------

    #[test]
    fn the_fallback_comment_is_signed_and_republishes_no_unvalidated_line() {
        // The fallback used to enumerate `review.comments` -- the UNVALIDATED
        // set -- and posted the body raw, so it republished the very line
        // numbers validation had just removed, unsigned. It now renders the
        // validated partition and goes through `crate::publish`.
        let review = ReviewResponse {
            summary: "two findings".to_string(),
            verdict: "REQUEST_CHANGES".to_string(),
            comments: vec![
                c("src/alpha.rs", 11, "RIGHT"),
                c("src/alpha.rs", 900, "RIGHT"),
            ],
        };
        let validation = validate_comments_against_diff(DIFF, &review.comments);
        let body = build_fallback_body(&review, &validation, "deadbeefcafe");

        assert!(
            crate::publish::is_signed(&body),
            "the fallback is published output and carries the mandatory signature: {body}"
        );
        assert!(body.contains("two findings"), "the summary must survive");
        assert!(
            body.contains("`src/alpha.rs:11`"),
            "the addressable finding must be published: {body}"
        );
        // The dropped finding is published too -- never silently discarded --
        // but only under its reason, never as an addressable location.
        let dropped_line = body
            .lines()
            .find(|l| l.contains("src/alpha.rs:900"))
            .unwrap_or_else(|| panic!("the dropped finding must still appear: {body}"));
        assert!(
            dropped_line.contains(DropReason::LineNotInDiffHunk.label()),
            "a dropped finding must be published with its reason: {dropped_line}"
        );
    }
}
