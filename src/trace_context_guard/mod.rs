use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod span_tracker;
pub use span_tracker::{DetachedSpanFinding, SpanTracker};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceContextReport {
    pub is_propagated: bool,
    pub tasks_scanned: usize,
    pub detached_findings: Vec<DetachedSpanFinding>,
    pub summary: String,
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

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            let lines: Vec<&str> = file_diff.lines().collect();
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

        let summary = if tasks_scanned == 0 && unresolved == 0 {
            // Nothing crossed a task boundary, so there is nothing to have a
            // view about, and the gate says exactly that rather than publishing
            // the word "verified" over a measurement it never took. The status
            // stays `Passed` -- see `src/coverage_guard.rs`, where a diff adding
            // no coverable line is `NothingToMeasure` and passes, rather than
            // `src/slo_canary_guard/mod.rs`, where a telemetry source that
            // should have been there was absent. This is the first kind: the
            // measurement is complete and it is empty.
            format!(
                "➖ NOTHING TO MEASURE (no task boundary in {SCOPE}; lines outside those hunks \
                 were not read)"
            )
        } else if tasks_scanned == 0 {
            format!(
                "➖ NOTHING TO MEASURE ({unresolved} task boundar{} in {SCOPE} open{s} in them and \
                 close{s} outside them, so none was classified)",
                if one { "y" } else { "ies" },
                s = if one { "s" } else { "" }
            )
        } else if is_propagated {
            format!(
                "✅ PASSED ({tasks_scanned} task boundar{} inspected in {SCOPE}; an \
                 `.instrument(...)` or `.in_current_span()` call appears inside the region each \
                 one opens{outside})",
                if tasks_scanned == 1 { "y" } else { "ies" }
            )
        } else {
            // The gate reads the lines this change adds *and the ones it
            // keeps*, because a region walk that skipped context lines could
            // bound nothing. So a location it names may be a line the author
            // only carried past, and the sentence says so rather than sending
            // them looking through their own edit for it.
            format!(
                "❌ FAILED ({} of {tasks_scanned} task boundaries inspected in {SCOPE} drop \
                 distributed tracing context: no `.instrument(...)` or `.in_current_span()` call \
                 appears inside the region they open{outside}) at {} -- a line named here may be \
                 one this change retained rather than one it wrote. {}",
                detached_findings.len(),
                located(&detached_findings),
                remedies(&detached_findings)
            )
        };

        Ok(TraceContextReport {
            is_propagated,
            tasks_scanned,
            detached_findings,
            summary,
        })
    }
}

/// How many detached boundaries the summary names before it stops counting.
const LOCATIONS_IN_SUMMARY: usize = 3;

/// `file:line` for the first few findings, in post-image coordinates.
///
/// `summary` is the only field of this report that reaches a published surface:
/// `src/pre_merge_guard/evaluator.rs` clones it into `GateStatus::Failed` and
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
/// found -- a thread is told to enter the caller's span inside its closure,
/// because `.instrument(...)` on an `FnOnce()` does not compile. Left in the
/// finding it would never be read: nothing renders `detached_findings`. There
/// are two of these strings and a diff rarely reaches both, so they are folded
/// in whole rather than summarised.
fn remedies(findings: &[DetachedSpanFinding]) -> String {
    let mut out: Vec<&str> = Vec::new();
    for finding in findings {
        if !out.contains(&finding.issue.as_str()) {
            out.push(&finding.issue);
        }
    }
    out.join(" ")
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
        if line.starts_with('-') {
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
