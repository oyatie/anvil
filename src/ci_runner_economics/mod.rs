use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod sku_allocator;
pub use sku_allocator::{RunnerSkuAllocator, RunnerSkuFinding};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerEconomicsReport {
    pub is_cost_optimal: bool,
    pub findings: Vec<RunnerSkuFinding>,
    pub summary: String,
}

pub struct CiRunnerEconomicsOptimizer {
    allocator: RunnerSkuAllocator,
}

impl CiRunnerEconomicsOptimizer {
    pub fn new() -> Self {
        let allocator = RunnerSkuAllocator::new();
        Self { allocator }
    }

    /// 100% Deterministic evaluation of GitHub Actions workflow runner SKU allocations and cost tiering
    pub fn evaluate_runner_economics(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<RunnerEconomicsReport> {
        info!(
            "Running CiRunnerEconomicsOptimizer (Deterministic Runner SKU Tiering & FinOps Routing) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings = Vec::new();

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            let lines: Vec<&str> = file_diff.lines().collect();
            let mut current_file = ".github/workflows/ci.yaml".to_string();
            if let Some(first_line) = lines.first() {
                if let Some(path) = first_line.split_whitespace().last() {
                    current_file = path.trim_start_matches("b/").to_string();
                }
            }

            let file_findings = self
                .allocator
                .scan_workflow_runners(&current_file, file_diff);
            findings.extend(file_findings);
        }

        let is_cost_optimal = findings.is_empty();
        let summary = if is_cost_optimal {
            "✅ PASSED (All CI runner allocations tier expensive multi-arch/macOS SKUs to merge trains)".to_string()
        } else {
            format!(
                "⚠️ CAUTION ({} un-tiered expensive runner SKU allocation(s) detected in PR triggers)",
                findings.len()
            )
        };

        Ok(RunnerEconomicsReport {
            is_cost_optimal,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_economics_nominal() {
        let opt = CiRunnerEconomicsOptimizer::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ runs-on: ubuntu-latest".to_string(),
            changed_files: vec![".github/workflows/pr.yaml".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = opt
            .evaluate_runner_economics(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_cost_optimal);
    }
}
