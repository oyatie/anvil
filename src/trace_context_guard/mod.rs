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

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            let lines: Vec<&str> = file_diff.lines().collect();
            let Some(path) = rust_path_of(&lines) else {
                continue;
            };

            let (scanned, findings) = self.tracker.scan(path, &shipped_lines(&lines));
            tasks_scanned += scanned;
            detached_findings.extend(findings);
        }

        let is_propagated = detached_findings.is_empty();
        let summary = if tasks_scanned == 0 {
            // Nothing crossed an async boundary, so there is nothing to have a
            // view about, and the gate says exactly that rather than publishing
            // the word "verified" over a measurement it never took. The status
            // stays `Passed` -- see `src/coverage_guard.rs`, where a diff adding
            // no coverable line is `NothingToMeasure` and passes, rather than
            // `src/slo_canary_guard/mod.rs`, where a telemetry source that
            // should have been there was absent. This is the first kind: the
            // measurement is complete and it is empty.
            "➖ NOTHING TO MEASURE (no async task boundary in the Rust lines this pull request adds \
             or keeps; nothing was inspected)"
                .to_string()
        } else if is_propagated {
            format!(
                "✅ PASSED ({} async task boundar{} inspected; each attaches a tracing span via `.instrument(...)`)",
                tasks_scanned,
                if tasks_scanned == 1 { "y" } else { "ies" }
            )
        } else {
            format!(
                "❌ FAILED ({} of {} async task boundaries inspected drop distributed tracing context: no `.instrument(...)` span is attached)",
                detached_findings.len(),
                tasks_scanned
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
