use crate::git_manager::diff_context::diffs_by_path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod buffer_limits;
pub use buffer_limits::{BufferLimitsChecker, UnboundedCapacityFinding};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstantWorkReport {
    pub is_bounded: bool,
    pub unbounded_findings: Vec<UnboundedCapacityFinding>,
    pub summary: String,
}

pub struct ConstantWorkGuard {
    checker: BufferLimitsChecker,
}

impl Default for ConstantWorkGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ConstantWorkGuard {
    pub fn new() -> Self {
        let checker = BufferLimitsChecker::new();
        Self { checker }
    }

    /// Evaluates PR diffs for constant-work and bounded capacity invariants
    pub fn evaluate_constant_work(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<ConstantWorkReport> {
        info!(
            "Running ConstantWorkGuard (Bounded Pools, Static Capacities & Anti-Fragility) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut unbounded_findings = Vec::new();

        for file in diffs_by_path(&diff_ctx.diff_content) {
            if !file.path.ends_with(".rs") {
                continue;
            }
            // `added`, not the whole hunk: an unbounded channel this change
            // REMOVES is not one it introduces, and the gate used to refuse
            // the pull request that replaced it with a bounded one.
            let findings = self
                .checker
                .scan_unbounded_structures(&file.path, file.added());
            unbounded_findings.extend(findings);
        }

        let is_bounded = unbounded_findings.is_empty();
        let summary = if is_bounded {
            "✅ PASSED (All channels, buffers, and pools enforce bounded capacities and backpressure)".to_string()
        } else {
            format!(
                "❌ FAILED ({} unbounded buffer/channel allocation(s) violate constant-work invariants)",
                unbounded_findings.len()
            )
        };

        Ok(ConstantWorkReport {
            is_bounded,
            unbounded_findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_work_passes_bounded() {
        let guard = ConstantWorkGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ let (tx, rx) = tokio::sync::mpsc::channel(1024);".to_string(),
            changed_files: vec!["src/queue.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard
            .evaluate_constant_work(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_bounded);
    }
}
