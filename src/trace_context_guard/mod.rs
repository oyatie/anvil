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
            if !file_diff.contains(".rs") {
                continue;
            }

            let lines: Vec<&str> = file_diff.lines().collect();
            for line in &lines {
                if line.contains("tokio::spawn") {
                    tasks_scanned += 1;
                }
            }

            let mut current_file = "unknown.rs".to_string();
            if let Some(first_line) = lines.first()
                && let Some(path) = first_line.split_whitespace().last()
            {
                current_file = path.trim_start_matches("b/").to_string();
            }

            let findings = self.tracker.scan_detached_tasks(&current_file, file_diff);
            detached_findings.extend(findings);
        }

        let is_propagated = detached_findings.is_empty();
        let summary = if is_propagated {
            format!(
                "✅ PASSED (W3C trace context & span instrumentation verified across {} async boundaries)",
                tasks_scanned
            )
        } else {
            format!(
                "❌ FAILED ({} detached async task(s) drop distributed tracing context)",
                detached_findings.len()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trace_guard_passes_clean_diff() {
        let guard = TraceContextGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ pub fn compute() -> i32 { 42 }".to_string(),
            changed_files: vec!["src/compute.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard
            .evaluate_trace_propagation(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_propagated);
    }
}
