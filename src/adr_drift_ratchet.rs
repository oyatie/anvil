use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdrReport {
    pub is_compliant: bool,
    pub adrs_evaluated: usize,
    pub scaffolded_adrs: Vec<String>,
    pub violations: Vec<String>,
    pub summary: String,
}

pub struct AdrDriftRatchet;

impl AdrDriftRatchet {
    pub fn new() -> Self {
        Self
    }

    /// Validates that architectural modifications carry valid 5-field ADR entries
    pub fn evaluate_adr_parity(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<AdrReport> {
        info!(
            "Running AdrDriftRatchet (Living Architecture Decision Record Ratchet) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut adrs_evaluated = 0;
        let mut scaffolded_adrs = Vec::new();
        let mut violations = Vec::new();

        // 1. Detect if PR introduces major architectural traits or new top-level components
        let has_arch_changes = diff_ctx.changed_files.iter().any(|f| {
            f.contains("/ports/")
                || f.contains("/adapters/")
                || f.contains("/facade/")
                || f.ends_with("lib.rs")
                || f.ends_with("schema.sql")
        });

        // 2. Check for ADR changes in docs/decisions/ or docs/adr/
        let adr_files: Vec<&String> = diff_ctx
            .changed_files
            .iter()
            .filter(|f| f.contains("docs/decisions/") || f.contains("docs/adr/"))
            .collect();

        if !adr_files.is_empty() {
            adrs_evaluated += adr_files.len();

            // Validate mandatory 5-field schema
            let required_fields = ["achieves", "origin", "rule", "ensure", "overturn_when"];
            for adr_file in &adr_files {
                for field in &required_fields {
                    let pattern = match *field {
                        "overturn_when" => r"(?i)\boverturn[-_ ]when\b".to_string(),
                        f => format!(r"(?i)\b{}\b", f),
                    };
                    let re = Regex::new(&pattern).unwrap();
                    if !re.is_match(&diff_ctx.diff_content) {
                        violations.push(format!(
                            "ADR `{}` is missing required architectural clause: `{}`",
                            adr_file, field
                        ));
                    }
                }
            }
        } else if has_arch_changes {
            // New architecture without ADR
            scaffolded_adrs.push(format!(
                "docs/decisions/ADR-{:04}-pr-{}.md",
                diff_ctx.pr_number, diff_ctx.pr_number
            ));
        }

        let is_compliant = violations.is_empty();
        let summary = if is_compliant {
            if !scaffolded_adrs.is_empty() {
                format!(
                    "✨ AUTO-SCAFFOLDED (Draft ADR generated for architectural changes: {:?})",
                    scaffolded_adrs
                )
            } else {
                "✅ PASSED (Architectural changes fully covered by 5-field ADR specifications)"
                    .to_string()
            }
        } else {
            format!(
                "❌ FAILED ({} ADR schema violation(s) detected)",
                violations.len()
            )
        };

        Ok(AdrReport {
            is_compliant,
            adrs_evaluated,
            scaffolded_adrs,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adr_drift_ratchet_valid_schema() {
        let ratchet = AdrDriftRatchet::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 829,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: r#"
+## ADR-0042: Shared Receipt Store
+### achieves: Single authority record
+### origin: console-yw0
+### rule: Bound writers via ReceiptOwner
+### ensure: Zero stale shared permissions
+### overturn_when: Single-writer DB role migration lands
"#
            .to_string(),
            changed_files: vec!["docs/decisions/ADR-0042.md".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = ratchet
            .evaluate_adr_parity(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_compliant);
        assert_eq!(rep.adrs_evaluated, 1);
    }
}
