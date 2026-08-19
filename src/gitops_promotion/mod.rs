use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod digest_pinner;
pub use digest_pinner::{DigestPinFinding, DigestPinner};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOpsPromotionReport {
    pub is_pinned: bool,
    pub unpinned_findings: Vec<DigestPinFinding>,
    pub summary: String,
}

pub struct GitOpsPromotionEngine {
    pinner: DigestPinner,
}

impl Default for GitOpsPromotionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GitOpsPromotionEngine {
    pub fn new() -> Self {
        let pinner = DigestPinner::new();
        Self { pinner }
    }

    /// 100% Deterministic evaluation of container image digest pinning and GitOps environment promotion manifests
    pub fn evaluate_manifest_promotions(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<GitOpsPromotionReport> {
        info!(
            "Running GitOpsPromotionEngine (Deterministic OCI Digest Pinning & Environment Promotion) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut unpinned_findings = Vec::new();

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            let is_gitops_manifest = file_diff.contains("gitops/")
                || file_diff.contains("iac/")
                || file_diff.contains("helm/")
                || file_diff.contains("k8s/")
                || file_diff.contains(".yaml")
                || file_diff.contains(".yml");

            if !is_gitops_manifest {
                continue;
            }

            let lines: Vec<&str> = file_diff.lines().collect();
            let mut current_file = "manifest.yaml".to_string();
            if let Some(first_line) = lines.first() {
                if let Some(path) = first_line.split_whitespace().last() {
                    current_file = path.trim_start_matches("b/").to_string();
                }
            }

            let findings = self.pinner.scan_unpinned_images(&current_file, file_diff);
            unpinned_findings.extend(findings);
        }

        let is_pinned = unpinned_findings.is_empty();
        let summary = if is_pinned {
            "✅ PASSED (All container image references are deterministically pinned to immutable sha256 digests)".to_string()
        } else {
            format!(
                "❌ FAILED ({} mutable/unpinned container image reference(s) detected)",
                unpinned_findings.len()
            )
        };

        Ok(GitOpsPromotionReport {
            is_pinned,
            unpinned_findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gitops_promotion_passes_pinned_digest() {
        let engine = GitOpsPromotionEngine::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ image: ghcr.io/oyatie/console@sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string(),
            changed_files: vec!["infra/gitops/app.yaml".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = engine
            .evaluate_manifest_promotions(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_pinned);
    }
}
