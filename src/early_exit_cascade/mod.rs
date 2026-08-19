use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod fast_checks;
pub use fast_checks::{FastChecksProber, FastPreflightFinding};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyExitReport {
    pub is_clean: bool,
    pub fast_findings: Vec<FastPreflightFinding>,
    pub summary: String,
}

pub struct EarlyExitCascadeGuard {
    prober: FastChecksProber,
}

impl EarlyExitCascadeGuard {
    pub fn new() -> Self {
        let prober = FastChecksProber::new();
        Self { prober }
    }

    /// 100% Deterministic evaluation of sub-second pre-flight static gates
    pub fn evaluate_preflight_cascade(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<EarlyExitReport> {
        info!(
            "Running EarlyExitCascadeGuard (Deterministic Sub-Second Pre-Flight Prober) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let fast_findings = self.prober.probe_static_invariants(&diff_ctx.diff_content);
        let is_clean = fast_findings.is_empty();

        let summary = if is_clean {
            "✅ PASSED (Sub-second pre-flight checks passed; matrix dispatched safely)".to_string()
        } else {
            format!(
                "❌ FAILED ({} static pre-flight blocker(s) detected; aborted runner matrix execution)",
                fast_findings.len()
            )
        };

        Ok(EarlyExitReport {
            is_clean,
            fast_findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_early_exit_nominal() {
        let guard = EarlyExitCascadeGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn clean() {}".to_string(),
            changed_files: vec!["src/clean.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard.evaluate_preflight_cascade(Path::new("."), &diff_ctx).unwrap();
        assert!(rep.is_clean);
    }
}
