use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod span_tracker;
pub use span_tracker::{DetachedSpanFinding, SpanTracker};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContextReport {
    pub is_propagated: bool,
    pub tasks_scanned: usize,
    pub detached_findings: Vec<DetachedSpanFinding>,
    pub summary: String,
    /// The verdict this guard reached, decided here and published unchanged.
    ///
    /// `src/slo_canary_guard/mod.rs` is the precedent: it builds its own
    /// `GateStatus` and `evaluator.rs` clones it. The alternative -- handing the
    /// evaluator a `bool` and letting it rebuild a two-valued status -- is the
    /// channel through which three of the four sentences this guard composes
    /// were lost, because `GateStatus::Passed` carries no string and
    /// `publish::scorecard::render` prints nothing for it. A gate that formats a
    /// sentence it knows is discarded is publishing the same unmeasured
    /// assurance it exists to remove.
    pub status: GateStatus,
}

pub struct TraceContextGuard {
    tracker: SpanTracker,
}

impl Default for TraceContextGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceContextGuard {
    pub fn new() -> Self {
        let tracker = SpanTracker::new();
        Self { tracker }
    }

    /// Evaluates PR diffs for W3C distributed tracing context propagation
    pub fn evaluate_trace_propagation(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<TraceContextReport> {
        info!(
            "Running TraceContextGuard (W3C Distributed Tracing & Span Invariant Guard) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut detached_findings = Vec::new();
        let mut tasks_scanned = 0;
        let mut unresolved = 0;

        for lines in file_chunks(&diff_ctx.diff_content) {
            let Some(path) = rust_path_of(&lines) else {
                continue;
            };

            // One scan per hunk, never one per file. A file's hunks are
            // disjoint windows onto it, so a region walk that ran from one into
            // the next would bound a boundary with a parenthesis from code
            // hundreds of lines away.
            for hunk in shipped_lines(&lines) {
                let outcome = self.tracker.scan(path, &hunk);
                tasks_scanned += outcome.classified;
                unresolved += outcome.unresolved;
                detached_findings.extend(outcome.detached);
            }
        }

        let is_propagated = detached_findings.is_empty();
        // Every sentence below is scoped to the evidence that was actually read.
        // The gate is handed diff hunks, never files, so a boundary this pull
        // request keeps outside a hunk is invisible to it -- and "the lines this
        // pull request adds or keeps", which the sentence used to say, is a
        // claim about the whole source. That is the same shape as the defect
        // this gate exists to close, stated more quietly.
        const SCOPE: &str = "the Rust diff hunks read";
        // A boundary whose parenthesis closes outside those hunks was never
        // measured; it is disclosed rather than folded into either verdict.
        let one = unresolved == 1;
        let outside = if unresolved == 0 {
            String::new()
        } else {
            format!(
                "; {unresolved} further boundar{} open{s} in them and close{s} outside them, so {} not classified",
                if one { "y" } else { "ies" },
                if one { "it was" } else { "they were" },
                s = if one { "s" } else { "" }
            )
        };

        let (status, summary) = if !is_propagated {
            // The gate reads the lines this change adds *and the ones it
            // keeps*, because a region walk that skipped context lines could
            // bound nothing. So a location it names may be a line the author
            // only carried past, and the sentence says so rather than sending
            // them looking through their own edit for it.
            //
            // The qualifier on the predicate is not decoration. `SpanTracker`
            // diverts the text of a nested boundary into that boundary's own
            // frame, so what it tested is "inside the region it opens, minus
            // the regions its nested boundaries own". Dropped, the sentence
            // asserts something stronger than the code measured -- on the one
            // surface an author reads while looking straight at the
            // `.instrument(` it says is not there.
            let summary = format!(
                "❌ FAILED ({} of {tasks_scanned} task boundar{} inspected in {SCOPE} drop \
                 distributed tracing context: no `.instrument(...)` or `.in_current_span()` call \
                 appears inside the region they open, outside the regions of the boundaries nested \
                 in them{outside}) at {} -- a line named here may be one this change retained \
                 rather than one it wrote. {}",
                detached_findings.len(),
                if tasks_scanned == 1 { "y" } else { "ies" },
                located(&detached_findings),
                remedies(&detached_findings)
            );
            (GateStatus::Failed(summary.clone()), summary)
        } else if unresolved > 0 {
            // A boundary that was *there* and could not be classified is not an
            // empty measurement. `src/coverage_guard.rs:139` maps
            // `NothingToMeasure` -- there was nothing to look at -- to `Passed`,
            // and a thing that was there and could not be measured to
            // `NotMeasured`; `src/slo_canary_guard/mod.rs` does the same. Routed
            // to the first shape, an uninstrumented spawn a pull request adds
            // ships green and undisclosed. `NotMeasured` publishes the count and
            // the reason and blocks merge-queue admission via
            // `PreMergeCertificationReport::is_admissible` (invariant I1),
            // without accusing the pull request of a defect.
            let inspected = if tasks_scanned == 0 {
                String::new()
            } else {
                format!(
                    "; {tasks_scanned} further boundar{} in them {} inspected and cleared",
                    if tasks_scanned == 1 { "y" } else { "ies" },
                    if tasks_scanned == 1 { "was" } else { "were" }
                )
            };
            let summary = format!(
                "➖ NOT MEASURED ({unresolved} task boundar{} in {SCOPE} open{s} in them and \
                 close{s} outside them, so {} seen and not judged{inspected})",
                if one { "y" } else { "ies" },
                if one { "it was" } else { "they were" },
                s = if one { "s" } else { "" }
            );
            (
                GateStatus::NotMeasured {
                    gate_id: "trace_status".to_string(),
                    reason: summary.clone(),
                },
                summary,
            )
        } else if tasks_scanned == 0 {
            // Nothing crossed a task boundary, so there is nothing to have a
            // view about, and the gate says exactly that rather than publishing
            // the word "verified" over a measurement it never took.
            //
            // `Warning` rather than `Passed`: `Passed` is a unit variant, so the
            // sentence would be discarded before any reader saw it, and the
            // scorecard would render the row as a bare tick counted in
            // "N/N gates passed" -- the claim issue #14 filed. `Warning` is
            // `is_acceptable()`, so it neither blocks the merge queue nor
            // accuses this change of anything; it does not follow
            // `src/slo_canary_guard/mod.rs` into `NotMeasured`, because a diff
            // that crosses no boundary is not absent evidence -- the evidence is
            // complete and it is empty.
            let summary = format!(
                "➖ NOTHING TO MEASURE (no task boundary in {SCOPE}; lines outside those hunks \
                 were not read)"
            );
            (GateStatus::Warning(summary.clone()), summary)
        } else {
            // Boundaries were inspected, every region was established, and every
            // one carries a span. This is the one arm entitled to be silent: the
            // scorecard enumerates findings and counts passes, and there is no
            // finding here.
            let summary = format!(
                "✅ PASSED ({tasks_scanned} task boundar{} inspected in {SCOPE}; an \
                 `.instrument(...)` or `.in_current_span()` call appears inside the region each \
                 one opens, outside the regions of the boundaries nested in it)",
                if tasks_scanned == 1 { "y" } else { "ies" }
            );
            (GateStatus::Passed, summary)
        };

        Ok(TraceContextReport {
            is_propagated,
            tasks_scanned,
            detached_findings,
            summary,
            status,
        })
    }
}

/// How many detached boundaries the summary names before it stops counting.
const LOCATIONS_IN_SUMMARY: usize = 3;

/// `file:line` for the first few findings, in post-image coordinates.
///
/// `summary` is the only field of this report that reaches a published surface:
/// it is the string the status carries, and `src/pre_merge_guard/evaluator.rs`
/// drops `detached_findings` on the floor. Without the locations folded in, an
/// author is told a count and nothing else -- not which file, not which line,
/// and not that the spawn may be a context line their change only retained.
///
/// Printed as `path:line`, it is a location claim, so the number has to be a
/// real line of the file after the merge -- see [`shipped_lines`].
fn located(findings: &[DetachedSpanFinding]) -> String {
    let mut parts: Vec<String> = findings
        .iter()
        .take(LOCATIONS_IN_SUMMARY)
        .map(|f| format!("{}:{}", f.file_path, f.line_number))
        .collect();
    if findings.len() > LOCATIONS_IN_SUMMARY {
        parts.push(format!(
            "and {} more",
            findings.len() - LOCATIONS_IN_SUMMARY
        ));
    }
    parts.join(", ")
}

/// Every distinct remedy the findings carry, in the order they first appear.
///
/// `DetachedSpanFinding::issue` says what to do about the kind of boundary it
/// found -- a closure-taking one is told to enter the caller's span inside the
/// closure, because the `Instrument` combinator does not apply to an `FnOnce()`.
/// Left in the finding it would never be read: nothing renders
/// `detached_findings`. There are two of these strings, and a line carrying one
/// of each has to publish both, so they are folded in whole rather than
/// summarised.
fn remedies(findings: &[DetachedSpanFinding]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for finding in findings {
        if !out.contains(&finding.issue.as_str()) {
            out.push(&finding.issue);
        }
    }
    out.join(" ")
}

/// The diff, cut into one chunk of lines per file it touches.
///
/// Anchored at a line boundary. Split on the bare substring `diff --git`, a
/// Rust file whose own body contains those words -- a one-line comment, a
/// string literal, a fixture in this repository's own suite -- is cut in two,
/// and the tail carries no `+++ ` header, so [`rust_path_of`] returns `None` and
/// the rest of that file is dropped without a word. The gate then publishes that
/// it found no boundary in a hunk that plainly contains one: absent evidence
/// rendered as a pass, which is the failure this gate exists to close, and a
/// one-line evasion available to any author on a watched repository.
///
/// Every body line of a diff carries a `+`, `-` or space prefix, so no body line
/// can begin with the separator and no real chunk can be missed.
fn file_chunks(diff: &str) -> Vec<Vec<&str>> {
    let mut chunks: Vec<Vec<&str>> = Vec::new();
    for line in diff.lines() {
        if line.starts_with("diff --git ") || chunks.is_empty() {
            chunks.push(Vec::new());
        }
        chunks
            .last_mut()
            .expect("a chunk is open before any line is read")
            .push(line);
    }
    chunks
}

/// The post-image path of a diff chunk, when it is a Rust file.
///
/// Read off the `+++ b/<path>` header rather than searched for in the chunk
/// text: four Markdown files in this repository name a `.rs` path in their
/// prose, and `file_diff.contains(".rs")` treats every one of them as Rust.
fn rust_path_of<'a>(lines: &[&'a str]) -> Option<&'a str> {
    let header = lines.iter().find_map(|l| l.strip_prefix("+++ "))?;
    let path = header.split_whitespace().next()?;
    let path = path.strip_prefix("b/").unwrap_or(path);
    path.ends_with(".rs").then_some(path)
}

/// The body lines of a diff chunk that this pull request ships -- added or left
/// untouched -- grouped by the hunk they belong to, each with the line it will
/// occupy in the file after the merge.
///
/// One `Vec` per hunk, because a hunk is the unit a region may be established
/// in: the hunks of a file are disjoint windows onto it, and a walk that ran
/// off the end of one and into the next would bound a boundary with a
/// parenthesis from unrelated code.
///
/// The line number is read off the `@@ -a,b +c,d @@` header and counted forward
/// over the lines the post-image keeps. Counted over the chunk instead -- which
/// is what shipped here before -- it numbers the `index`, `---`, `+++` and `@@`
/// headers too and never restarts, so it is neither a source line nor a
/// hunk-relative one, and `located` prints it to a reviewer as `path:line`.
///
/// A removed line is not code the author ships: counting it reports boundaries
/// that were inspected but no longer exist, and reading it lets a `.instrument(`
/// on its way out clear a spawn that ships detached. It does not advance the
/// post-image line either.
///
/// A chunk with no `@@` header declares no position, so nothing in it is read.
/// Every diff git produces carries one; the gate does not guess where a body
/// sits in order to publish a location for it.
fn shipped_lines<'a>(lines: &[&'a str]) -> Vec<Vec<(usize, &'a str)>> {
    let mut hunks: Vec<Vec<(usize, &'a str)>> = Vec::new();
    let mut at: Option<usize> = None;

    for line in lines {
        if let Some(start) = post_image_start(line) {
            hunks.push(Vec::new());
            at = Some(start);
            continue;
        }
        // Anything before the first `@@` is header, not body.
        let Some(next) = at.as_mut() else { continue };
        // A removed line is not code the author ships. Nor is the marker
        // `\ No newline at end of file`: git writes it mid-hunk whenever the
        // pre-image's final line lacked a trailing newline and is being
        // replaced, it occupies no position in either image, and numbering it
        // pushes every later location in that hunk one line up -- past the end
        // of the file, on a short post-image -- and hands its own text to the
        // scanner as live code.
        if line.starts_with('-') || line.starts_with('\\') {
            continue;
        }
        // Never sliced by byte: a line may be empty, and this corpus carries
        // Rust files whose first character is multi-byte.
        let text = line
            .strip_prefix('+')
            .or_else(|| line.strip_prefix(' '))
            .unwrap_or(line);
        hunks
            .last_mut()
            .expect("a hunk was opened before any body line was read")
            .push((*next, text));
        *next += 1;
    }

    hunks
}

/// The post-image line a hunk header says its body begins at: the `c` of
/// `@@ -a,b +c,d @@`.
fn post_image_start(line: &str) -> Option<usize> {
    let after_old = line.strip_prefix("@@ -")?;
    let after_new = after_old.split_once('+')?.1;
    after_new
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

// `test_trace_guard_passes_clean_diff` used to live here. It fed the gate a
// diff containing no `.rs` chunk -- so the scan loop never ran, nothing was
// inspected -- and then asserted `rep.is_propagated`. Issue #14 cites it as the
// test that exercises the defective path: it did not check that the gate was
// right, it recorded that the gate said "verified" having looked at nothing,
// and it fixed `is_propagated == true` as the answer for the nothing-in-scope
// case, which is precisely the question this lane leaves open for the owner.
//
// The behaviours it was reaching for -- and the ones it could not see -- are in
// `tests/trace_gate_claims_only_what_it_measured_test.rs`.
