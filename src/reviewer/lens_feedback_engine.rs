use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CanonicalLens {
    CartesianDoubt,
    EssentialismYagni,
    ChestertonsFence,
    Contrarian10x,
    SocraticInquiries,
    Pragmatism,
    RedTeamThreatModel,
    SystemsThinking,
    OperabilityDay2,
    OpportunityCost,
    BlastRadius,
    AntiFragility,
    SharedNothing,
    FinOpsUnitCost,
    TelemetryFirst,
    ZeroTrustDefenseInDepth,
}

impl CanonicalLens {
    pub fn name(&self) -> &'static str {
        match self {
            CanonicalLens::CartesianDoubt => "1. Cartesian Doubt",
            CanonicalLens::EssentialismYagni => "2. Essentialism / YAGNI",
            CanonicalLens::ChestertonsFence => "3. Chesterton's Fence",
            CanonicalLens::Contrarian10x => "4. Contrarian / 10x",
            CanonicalLens::SocraticInquiries => "5. Socratic Inquiries",
            CanonicalLens::Pragmatism => "6. Pragmatism",
            CanonicalLens::RedTeamThreatModel => "7. Red Team / Threat Modeling",
            CanonicalLens::SystemsThinking => "8. Systems Thinking",
            CanonicalLens::OperabilityDay2 => "9. Operability / Day-2",
            CanonicalLens::OpportunityCost => "10. Opportunity Cost",
            CanonicalLens::BlastRadius => "11. Blast-radius Containment",
            CanonicalLens::AntiFragility => "12. Anti-fragility",
            CanonicalLens::SharedNothing => "13. Shared-nothing Architecture",
            CanonicalLens::FinOpsUnitCost => "14. FinOps / Unit-cost",
            CanonicalLens::TelemetryFirst => "15. Telemetry-first",
            CanonicalLens::ZeroTrustDefenseInDepth => "16. Zero-trust / Defense-in-Depth",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LensFindingSeverity {
    Info,
    Caution,
    CriticalViolation,
    ResolvedCompliant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LensEvaluationFinding {
    pub lens: CanonicalLens,
    pub severity: LensFindingSeverity,
    pub description: String,
    pub resolution_receipt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LensFeedbackReport {
    pub total_lenses_evaluated: usize,
    pub critical_violations: usize,
    pub resolved_findings: usize,
    pub is_pre_merge_admissible: bool,
    pub findings: Vec<LensEvaluationFinding>,
    pub summary: String,
}

pub struct LensFeedbackEngine;

impl LensFeedbackEngine {
    /// Ingests 16-lens review findings, checks active ADRs & compliance doctrine, and accounts results back into pipeline
    pub fn reconcile_lens_findings(
        repo_dir: &Path,
        raw_review_text: &str,
        pr_number: u64,
    ) -> Result<LensFeedbackReport> {
        info!(
            "🧠 [Lens Feedback Engine] Accounting 16-Lens Canonical Matrix findings back into pipeline for PR #{}...",
            pr_number
        );

        let mut findings = Vec::new();
        let decisions_dir = repo_dir.join("docs/decisions");
        // Parse all 16 canonical lenses in review output
        for (lens, pattern, adr_key) in [
            (CanonicalLens::CartesianDoubt, "Cartesian Doubt", "ADR-0701"),
            (CanonicalLens::EssentialismYagni, "Essentialism", "ADR-0702"),
            (
                CanonicalLens::ChestertonsFence,
                "Chesterton's Fence",
                "ADR-0709",
            ),
            (CanonicalLens::Contrarian10x, "Contrarian", "ADR-0703"),
            (
                CanonicalLens::SocraticInquiries,
                "Socratic Inquiries",
                "ADR-0704",
            ),
            (CanonicalLens::Pragmatism, "Pragmatism", "ADR-0705"),
            (CanonicalLens::RedTeamThreatModel, "Red Team", "ADR-0711"),
            (
                CanonicalLens::SystemsThinking,
                "Systems Thinking",
                "ADR-0706",
            ),
            (CanonicalLens::OperabilityDay2, "Operability", "ADR-0714"),
            (
                CanonicalLens::OpportunityCost,
                "Opportunity Cost",
                "ADR-0707",
            ),
            (CanonicalLens::BlastRadius, "Blast-radius", "ADR-0712"),
            (CanonicalLens::AntiFragility, "Anti-fragility", "ADR-0708"),
            (CanonicalLens::SharedNothing, "Shared-nothing", "ADR-0715"),
            (CanonicalLens::FinOpsUnitCost, "FinOps", "ADR-0716"),
            (CanonicalLens::TelemetryFirst, "Telemetry-first", "ADR-0713"),
            (
                CanonicalLens::ZeroTrustDefenseInDepth,
                "Zero-trust",
                "ADR-0710",
            ),
        ] {
            if raw_review_text.contains(pattern) {
                let is_critical = raw_review_text.contains("CRITICAL VIOLATION")
                    || raw_review_text.contains("🛑")
                    || raw_review_text.contains("REQUEST_CHANGES");

                if is_critical {
                    // Check if PR explicitly cites an authoritative ADR that resolves this lens finding
                    let explicitly_cited = raw_review_text.contains(adr_key);
                    let adr_exists_on_disk = decisions_dir.exists()
                        && std::fs::read_dir(&decisions_dir)
                            .map(|entries| {
                                entries
                                    .filter_map(|e| e.ok())
                                    .any(|e| e.file_name().to_string_lossy().contains(adr_key))
                            })
                            .unwrap_or(false);

                    let has_resolving_adr = explicitly_cited && adr_exists_on_disk;

                    if has_resolving_adr {
                        findings.push(LensEvaluationFinding {
                            lens,
                            severity: LensFindingSeverity::ResolvedCompliant,
                            description: format!(
                                "Finding on '{}' reconciled and authorized under living architecture record {}",
                                lens.name(),
                                adr_key
                            ),
                            resolution_receipt: Some(format!(
                                "ANVIL-LENS-RESOLVED-RECEIPT#{}-{}",
                                pr_number, adr_key
                            )),
                        });
                    } else {
                        findings.push(LensEvaluationFinding {
                            lens,
                            severity: LensFindingSeverity::CriticalViolation,
                            description: format!(
                                "Unresolved critical finding on '{}'. Authoritative ADR ({}) citation required.",
                                lens.name(),
                                adr_key
                            ),
                            resolution_receipt: None,
                        });
                    }
                }
            }
        }

        let critical_count = findings
            .iter()
            .filter(|f| f.severity == LensFindingSeverity::CriticalViolation)
            .count();
        let resolved_count = findings
            .iter()
            .filter(|f| f.severity == LensFindingSeverity::ResolvedCompliant)
            .count();

        let is_admissible = critical_count == 0;
        let summary = if is_admissible {
            format!(
                "✅ 16-Lens Matrix Compliant: {} findings resolved via living ADR doctrine; 0 blocking violations.",
                resolved_count
            )
        } else {
            format!(
                "❌ 16-Lens Matrix Blocked: {} critical violation(s) require resolution before merge queue admission.",
                critical_count
            )
        };

        Ok(LensFeedbackReport {
            total_lenses_evaluated: 16,
            critical_violations: critical_count,
            resolved_findings: resolved_count,
            is_pre_merge_admissible: is_admissible,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chestertons_fence_reconciled_with_adr() {
        let temp_dir = std::env::temp_dir().join("anvil_lens_test");
        let dec_dir = temp_dir.join("docs/decisions");
        let _ = std::fs::create_dir_all(&dec_dir);
        let _ = std::fs::write(
            dec_dir.join("ADR-0709-consolidate-attestations.md"),
            "# ADR-0709",
        );

        let review_text = "Review finding: 🛑 CRITICAL VIOLATION on Chesterton's Fence. Receipt deleted under ADR-0709.";
        let rep = LensFeedbackEngine::reconcile_lens_findings(&temp_dir, review_text, 5).unwrap();

        assert!(rep.is_pre_merge_admissible);
        assert_eq!(rep.resolved_findings, 1);
        assert_eq!(rep.critical_violations, 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
