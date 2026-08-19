use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod orphan_sweeper;
pub use orphan_sweeper::{OrphanManifestFinding, OrphanSweeper};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOpsDriftReport {
    pub is_safe: bool,
    pub orphan_findings: Vec<OrphanManifestFinding>,
    pub summary: String,
}

pub struct GitOpsDriftReconciler {
    sweeper: OrphanSweeper,
}

impl GitOpsDriftReconciler {
    pub fn new() -> Self {
        let sweeper = OrphanSweeper::new();
        Self { sweeper }
    }

    /// 100% Deterministic evaluation of ArgoCD / Flux ApplicationSet lifecycle and orphan drift
    pub fn evaluate_gitops_drift(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<GitOpsDriftReport> {
        info!(
            "Running GitOpsDriftReconciler (Deterministic Manifest Parity & Orphan Prevention) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let orphan_findings = self
            .sweeper
            .scan_orphan_risk(&diff_ctx.changed_files, &diff_ctx.diff_content);
        let is_safe = orphan_findings.is_empty();

        let summary = if is_safe {
            "✅ PASSED (All GitOps ApplicationSets, Kustomizations, and Helm values maintain declarative integrity)".to_string()
        } else {
            format!(
                "⚠️ CAUTION ({} unmanaged/unsafe GitOps manifest deletion(s) detected)",
                orphan_findings.len()
            )
        };

        Ok(GitOpsDriftReport {
            is_safe,
            orphan_findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gitops_drift_reconciler_nominal() {
        let rec = GitOpsDriftReconciler::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ replicaCount: 3".to_string(),
            changed_files: vec!["infra/gitops/values.yaml".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = rec
            .evaluate_gitops_drift(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_safe);
    }
}
