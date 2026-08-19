use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod signature_scanner;
pub use signature_scanner::{BreakingAbiFinding, SignatureScanner};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticAbiReport {
    pub is_abi_stable: bool,
    pub breaking_findings: Vec<BreakingAbiFinding>,
    pub summary: String,
}

pub struct SemanticAbiRatchet {
    scanner: SignatureScanner,
}

impl SemanticAbiRatchet {
    pub fn new() -> Self {
        let scanner = SignatureScanner::new();
        Self { scanner }
    }

    /// 100% Deterministic evaluation of public library ABI stability and breaking changes
    pub fn evaluate_abi_stability(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<SemanticAbiReport> {
        info!(
            "Running SemanticAbiRatchet (Deterministic Public ABI Stability & Semver Gate) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut breaking_findings = Vec::new();

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            let lines: Vec<&str> = file_diff.lines().collect();
            let mut current_file = "src/lib.rs".to_string();
            if let Some(first_line) = lines.first() {
                if let Some(path) = first_line.split_whitespace().last() {
                    current_file = path.trim_start_matches("b/").to_string();
                }
            }

            let file_findings = self.scanner.scan_abi_diff(&current_file, file_diff);
            breaking_findings.extend(file_findings);
        }

        let is_abi_stable = breaking_findings.is_empty();
        let summary = if is_abi_stable {
            "✅ PASSED (Public library API signatures & ABI layouts are backward-compatible)"
                .to_string()
        } else {
            format!(
                "❌ FAILED ({} breaking public ABI change(s) detected without semver major bump)",
                breaking_findings.len()
            )
        };

        Ok(SemanticAbiReport {
            is_abi_stable,
            breaking_findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abi_ratchet_nominal() {
        let ratchet = SemanticAbiRatchet::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ pub fn get_version() -> &'static str { \"1.0\" }".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = ratchet
            .evaluate_abi_stability(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_abi_stable);
    }
}
