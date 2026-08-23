use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod workspace_dag;
pub use workspace_dag::{WorkspaceDagSelector, WorkspacePackage};

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "predictive_test_status";

const NO_WORKSPACE_DISCOVERED: &str = "no workspace packages were discovered, so no selection was \
     pruned and nothing was measured";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveTestReport {
    pub status: GateStatus,
    pub is_optimized: bool,
    pub selected_packages: Vec<String>,
    pub skipped_packages_count: usize,
    pub pruning_ratio: f64,
    pub summary: String,
}

pub struct PredictiveTestSelector {
    dag_selector: WorkspaceDagSelector,
}

impl Default for PredictiveTestSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl PredictiveTestSelector {
    pub fn new() -> Self {
        let dag_selector = WorkspaceDagSelector::new();
        Self { dag_selector }
    }

    /// 100% Deterministic selection of affected test targets across workspace dependency DAG
    pub fn evaluate_test_selection(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<PredictiveTestReport> {
        info!(
            "Running PredictiveTestSelector (Deterministic DAG Test Selection) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        // Discovery used to fall back to a hand-written "anvil" package when it
        // found none, so the selector always had something to select and the
        // gate always had something to report. An undiscovered workspace is not
        // a one-package workspace.
        let workspace_packages = WorkspaceDagSelector::discover_workspace_packages_sync(_repo_dir);
        if workspace_packages.is_empty() {
            return Ok(PredictiveTestReport {
                status: GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason: NO_WORKSPACE_DISCOVERED.to_string(),
                },
                is_optimized: false,
                selected_packages: Vec::new(),
                skipped_packages_count: 0,
                pruning_ratio: 0.0,
                summary: NO_WORKSPACE_DISCOVERED.to_string(),
            });
        }

        let selected = self
            .dag_selector
            .select_affected_packages(&diff_ctx.changed_files, &workspace_packages);
        let total_packages = workspace_packages.len();
        let skipped = total_packages.saturating_sub(selected.len());
        let pruning_ratio =
            WorkspaceDagSelector::calculate_pruning_ratio(selected.len(), total_packages);
        // The gate measures selection, not wall-clock: nothing here times a
        // test run, so "optimized" can only mean that the selection is a
        // strict subset of the workspace. It used to be the literal `true`.
        let is_optimized = skipped > 0;
        let summary = format!(
            "Predictive selection targeted {} of {} packages, sparing {} (pruning ratio {:.1}%); no test run was timed",
            selected.len(),
            total_packages,
            skipped,
            pruning_ratio * 100.0
        );

        Ok(PredictiveTestReport {
            status: if is_optimized {
                GateStatus::Passed
            } else {
                GateStatus::Warning(summary.clone())
            },
            is_optimized,
            selected_packages: selected,
            skipped_packages_count: skipped,
            pruning_ratio,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predictive_selector_nominal() {
        let sel = PredictiveTestSelector::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn test() {}".to_string(),
            changed_files: vec!["src/main.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = sel
            .evaluate_test_selection(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_optimized);
    }
}
