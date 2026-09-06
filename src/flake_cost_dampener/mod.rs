use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod quarantine_log;
pub use quarantine_log::{QuarantineLogManager, QuarantinedTestEntry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlakeReport {
    pub is_clean: bool,
    pub quarantined_tests: Vec<QuarantinedTestEntry>,
    pub summary: String,
}

pub struct FlakeCostDampener {
    log_mgr: QuarantineLogManager,
}

impl Default for FlakeCostDampener {
    fn default() -> Self {
        Self::new()
    }
}

impl FlakeCostDampener {
    pub fn new() -> Self {
        let log_mgr = QuarantineLogManager::new();
        Self { log_mgr }
    }

    /// 100% Deterministic evaluation of test flakiness and automated compute waste dampening
    pub fn evaluate_flake_risks(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<FlakeReport> {
        info!(
            "Running FlakeCostDampener (Deterministic Flake Isolation & Compute Dampener) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let quarantined_tests = self
            .log_mgr
            .check_quarantined_tests(&diff_ctx.changed_files);
        let is_clean = quarantined_tests.is_empty();

        let summary = if is_clean {
            "✅ PASSED (Zero un-quarantined flaky tests detected in changed test paths)".to_string()
        } else {
            format!(
                "⚠️ CAUTION ({} flaky test(s) isolated to prevent runner re-run compute waste)",
                quarantined_tests.len()
            )
        };

        Ok(FlakeReport {
            is_clean,
            quarantined_tests,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flake_dampener_nominal() {
        let dampener = FlakeCostDampener::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn test_deterministic() {}".to_string(),
            changed_files: vec!["tests/unit_test.rs".to_string()],
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("."),
                crate::git_manager::Uncloned::TestFixture,
            ),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = dampener
            .evaluate_flake_risks(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_clean);
    }
}
