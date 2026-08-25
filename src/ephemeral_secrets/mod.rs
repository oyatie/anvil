use crate::git_manager::diff_context::diffs_by_path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod oidc_validator;
pub use oidc_validator::{OidcPolicyValidator, SecretPolicyFinding};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretPolicyReport {
    pub is_zero_trust: bool,
    pub findings: Vec<SecretPolicyFinding>,
    pub summary: String,
}

pub struct EphemeralSecretInjector {
    validator: OidcPolicyValidator,
}

impl Default for EphemeralSecretInjector {
    fn default() -> Self {
        Self::new()
    }
}

impl EphemeralSecretInjector {
    pub fn new() -> Self {
        let validator = OidcPolicyValidator::new();
        Self { validator }
    }

    /// 100% Deterministic evaluation of OIDC zero-trust credential policies
    pub fn evaluate_secret_policies(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<SecretPolicyReport> {
        info!(
            "Running EphemeralSecretInjector (Deterministic OIDC Zero-Trust Credential Gate) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings = Vec::new();

        for file in diffs_by_path(&diff_ctx.diff_content) {
            // The path is the one the diff states. It used to default to the
            // literal ".github/workflows/deploy.yaml", a plausible path this gate published
            // as the location of a finding that was not found there.
            //
            // `all` -- additions plus the context they sit in, removals excluded. The
            // rule asks what the file says after this change, and a line the
            // change DELETES is not part of that.

            let file_findings = self
                .validator
                .validate_workflow_secrets(&file.path, &file.all);
            findings.extend(file_findings);
        }

        let is_zero_trust = findings.is_empty();
        let summary = if is_zero_trust {
            "✅ PASSED (All CI credentials use short-lived OIDC federated tokens; zero static secrets)".to_string()
        } else {
            format!(
                "❌ FAILED ({} static long-lived credential violation(s) detected)",
                findings.len()
            )
        };

        Ok(SecretPolicyReport {
            is_zero_trust,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_injector_nominal() {
        let injector = EphemeralSecretInjector::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ permissions:\n+   id-token: write".to_string(),
            changed_files: vec![".github/workflows/deploy.yaml".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = injector
            .evaluate_secret_policies(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_zero_trust);
    }
}
