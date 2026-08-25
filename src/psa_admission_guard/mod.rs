use crate::git_manager::diff_context::diffs_by_path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod psa_rules;
pub use psa_rules::{PsaAdmissionRules, PsaPolicyFinding};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsaAdmissionReport {
    pub is_compliant: bool,
    pub findings: Vec<PsaPolicyFinding>,
    pub summary: String,
}

pub struct PsaAdmissionGuard {
    rules: PsaAdmissionRules,
}

impl Default for PsaAdmissionGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl PsaAdmissionGuard {
    pub fn new() -> Self {
        let rules = PsaAdmissionRules::new();
        Self { rules }
    }

    /// 100% Deterministic evaluation of Native Kubernetes Pod Security Admission (PSA) per ADR-0710 D-1
    pub fn evaluate_psa_admission(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<PsaAdmissionReport> {
        info!(
            "Running PsaAdmissionGuard (Deterministic Native Kubernetes PSA ADR-0710 D-1 Gate) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings = Vec::new();

        for file in diffs_by_path(&diff_ctx.diff_content) {
            // The path is the one the diff states. It used to default to the
            // literal "infra/ns.yaml", a plausible path this gate published
            // as the location of a finding that was not found there.
            //
            // `all` -- additions plus the context they sit in, removals excluded. The
            // rule asks what the file says after this change, and a line the
            // change DELETES is not part of that.

            let file_findings = self
                .rules
                .evaluate_psa_manifest(&file.path, file.after_change());
            findings.extend(file_findings);
        }

        let is_compliant = findings.is_empty();
        let summary = if is_compliant {
            "✅ PASSED (Native Kubernetes Pod Security Admission compliant: enforce: restricted or recorded exception)".to_string()
        } else {
            format!(
                "❌ FAILED ({} Native PSA violation(s) detected per ADR-0710 D-1)",
                findings.len()
            )
        };

        Ok(PsaAdmissionReport {
            is_compliant,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_psa_guard_nominal() {
        let guard = PsaAdmissionGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ pod-security.kubernetes.io/enforce: restricted".to_string(),
            changed_files: vec!["infra/ns.yaml".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard
            .evaluate_psa_admission(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_compliant);
    }
}
