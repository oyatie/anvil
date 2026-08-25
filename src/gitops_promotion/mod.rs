use crate::git_manager::diff_context::diffs_by_path;
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

        for file in diffs_by_path(&diff_ctx.diff_content) {
            // Scope is decided by the PATH, not by whether the word ".yaml"
            // appears somewhere in the text. The old predicate ran
            // `file_diff.contains(".yaml")` over the chunk's content, so a Rust
            // file that merely mentioned a manifest was scanned as one -- and
            // the finding was then filed against the literal `manifest.yaml`,
            // a plausible path that named nothing in the change.
            let is_gitops_manifest = ["gitops/", "iac/", "helm/", "k8s/"]
                .iter()
                .any(|d| file.path.contains(d))
                || file.path.ends_with(".yaml")
                || file.path.ends_with(".yml");

            if !is_gitops_manifest {
                continue;
            }

            // `all`: an image pinned by a line this change does not touch is
            // still pinned, and one it DELETES is not this change's mutable tag.
            let findings = self.pinner.scan_unpinned_images(&file.path, &file.all);
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
