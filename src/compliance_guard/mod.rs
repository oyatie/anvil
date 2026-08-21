use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

pub mod engine;
pub mod registry;
pub mod upstream_sync;

pub use engine::{RegulatoryEngine, StatutoryViolation};
pub use upstream_sync::UpstreamRegulatorySync;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceGuardReport {
    pub is_compliant: bool,
    pub violations: Vec<StatutoryViolation>,
    pub evaluation_date: String,
    pub jurisdictions_evaluated: Vec<String>,
    pub active_rules_count: usize,
    pub summary: String,
}

pub struct ComplianceGuard {
    engine: RegulatoryEngine,
    sync: UpstreamRegulatorySync,
}

impl Default for ComplianceGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplianceGuard {
    pub fn new() -> Self {
        Self {
            engine: RegulatoryEngine::new(),
            sync: UpstreamRegulatorySync::new(),
        }
    }

    /// Hot-reloads and syncs upstream regulatory rule definitions from an external directory
    pub fn sync_upstream_rules(&self, dir: &Path) -> Result<usize> {
        self.sync.sync_from_directory(dir)
    }

    /// Evaluates multi-jurisdiction, temporal, and internal corporate regulatory compliance
    pub fn evaluate_compliance(&self, diff_ctx: &PrDiffContext) -> Result<ComplianceGuardReport> {
        let current_date = "2026-08-19"; // Canonical platform time
        info!(
            "Running Dynamic Regulatory Compliance Guard (Temporal Date: {}) on {}#{}...",
            current_date, diff_ctx.repo, diff_ctx.pr_number
        );

        let enforceable_rules = self.sync.get_enforceable_rules(current_date);
        let active_rules_count = enforceable_rules.len();
        let violations = self.engine.scan_diff(diff_ctx, &enforceable_rules)?;

        let jurisdictions_evaluated = vec![
            "Korea (PIPA, 망법, 신정법, 전상법, 전금법, AI기본법)".to_string(),
            "United States (HIPAA, CCPA/CPRA, COPPA, FTC Act §5)".to_string(),
            "European Union (GDPR, EU AI Act, DORA, NIS2)".to_string(),
            "Global Standards (PCI-DSS v4.0.1, SOC 2, ISO 27001)".to_string(),
            "Internal Corporate Doctrine (Oyatie Architecture Decision Records)".to_string(),
        ];

        let has_blocking_violations = violations
            .iter()
            .any(|v| v.severity == "CRITICAL" || v.severity == "HIGH");
        let is_compliant = !has_blocking_violations;

        let summary = if is_compliant {
            format!(
                "Dynamic regulatory compliance verified: 100% compliant across {} enforceable statutory and internal rules (evaluated for {}).",
                active_rules_count, current_date
            )
        } else {
            format!(
                "Regulatory & statutory violations detected ({} violation(s)): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{}: {} [{}]", v.citation, v.title, v.line_snippet))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(ComplianceGuardReport {
            is_compliant,
            violations,
            evaluation_date: current_date.to_string(),
            jurisdictions_evaluated,
            active_rules_count,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_korean_rrn_under_pipa() {
        let guard = ComplianceGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 100,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ const testUserRRN = \"931225-1029384\";".to_string(),
            changed_files: vec!["src/test.ts".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_compliance(&diff_ctx).expect("Evaluates");
        assert!(!report.is_compliant);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].rule_id, "KR_PIPA_RRN_BAN");
        assert!(report.violations[0].citation.contains("개보법 §24의2"));
    }

    #[test]
    fn test_detects_ecom_dark_pattern_precheck() {
        let guard = ComplianceGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 101,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ <input type=\"checkbox\" defaultChecked={true} name=\"marketing\" />"
                .to_string(),
            changed_files: vec!["src/Checkout.tsx".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_compliance(&diff_ctx).expect("Evaluates");
        assert!(!report.is_compliant);
        assert_eq!(
            report.violations[0].rule_id,
            "KR_ECOM_ANTI_DARK_PATTERN_PRECHECK"
        );
        assert!(report.violations[0].citation.contains("전상법 §21의2"));
    }

    #[test]
    fn test_detects_global_pci_dss_plaintext_pan() {
        let guard = ComplianceGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 102,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ const leakedPan = \"4111111111111111\";".to_string(),
            changed_files: vec!["src/billing.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_compliance(&diff_ctx).expect("Evaluates");
        assert!(!report.is_compliant);
        assert_eq!(report.violations[0].rule_id, "GLOBAL_PCI_PLAINTEXT_PAN");
        assert!(report.violations[0].citation.contains("PCI-DSS"));
    }

    #[test]
    fn test_clean_diff_passes() {
        let guard = ComplianceGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 103,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ let masked = mask_token(&user.ci_token);".to_string(),
            changed_files: vec!["src/mask.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_compliance(&diff_ctx).expect("Evaluates");
        assert!(report.is_compliant);
        assert!(report.violations.is_empty());
        assert_eq!(report.jurisdictions_evaluated.len(), 5);
    }
}
