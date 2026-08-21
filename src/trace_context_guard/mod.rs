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

            let outcome = self.tracker.scan(path, &shipped_lines(&lines));
            tasks_scanned += outcome.classified;
            unresolved += outcome.unresolved;
            detached_findings.extend(outcome.detached);
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
            format!(
                "❌ FAILED ({} of {tasks_scanned} task boundaries inspected in {SCOPE} drop \
                 distributed tracing context: no `.instrument(...)` or `.in_current_span()` call \
                 appears inside the region they open{outside}) at {}",
                detached_findings.len(),
                located(&detached_findings)
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

/// `file:line` for the first few findings.
///
/// `summary` is the only field of this report that reaches a published surface:
/// `src/pre_merge_guard/evaluator.rs` clones it into `GateStatus::Failed` and
/// drops `detached_findings` on the floor. Without the locations folded in, an
/// author is told a count and nothing else -- not which file, not which line,
/// and not that the spawn may be a context line their change only retained.
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

/// The body lines of a diff chunk that this pull request ships -- added or
/// left untouched -- each with the position it occupies in the chunk.
///
/// A removed line is not code the author ships: counting it reports boundaries
/// that were inspected but no longer exist, and reading it lets a `.instrument(`
/// on its way out clear a spawn that ships detached.
fn shipped_lines<'a>(lines: &[&'a str]) -> Vec<(usize, &'a str)> {
    let mut body = Vec::new();
    let mut in_body = false;

    for (idx, line) in lines.iter().enumerate() {
        if line.starts_with("@@") {
            in_body = true;
            continue;
        }
        if !in_body {
            // `--- a/<path>` precedes it, so the header block ends here.
            in_body = line.starts_with("+++ ");
            continue;
        }
        if line.starts_with('-') {
            continue;
        }
        // Never sliced by byte: a line may be empty, and this corpus carries
        // Rust files whose first character is multi-byte.
        let text = line
            .strip_prefix('+')
            .or_else(|| line.strip_prefix(' '))
            .unwrap_or(line);
        body.push((idx + 1, text));
    }

    body
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
