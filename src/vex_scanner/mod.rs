use serde::{Deserialize, Serialize};

pub mod callgraph_pruner;
pub use callgraph_pruner::{CallgraphPruner, OpenVexStatement, VexImpactStatus};

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "openvex_status";

const MISSING_ADVISORY_SOURCE: &str = "no advisory feed or dependency inventory was read, so no CVE \
     reachability was scanned for this pull request";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenVexReport {
    pub status: GateStatus,
    pub passed: bool,
    pub statements: Vec<OpenVexStatement>,
}

#[derive(Debug, Clone, Default)]
pub struct OpenVexReachabilityScanner {
    pruner: CallgraphPruner,
}

impl OpenVexReachabilityScanner {
    pub fn new() -> Self {
        Self {
            pruner: CallgraphPruner::new(),
        }
    }

    /// The gate's answer when no advisory source was consulted.
    ///
    /// The pipeline passed the placeholders `"CVE-NONE"` / `"symbol_none"`,
    /// and the scanner clears anything whose symbol is absent from the diff,
    /// so every PR was attested NotAffected by a CVE that does not exist.
    pub fn evaluate_without_advisory_source(&self) -> OpenVexReport {
        OpenVexReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_ADVISORY_SOURCE.to_string(),
            },
            passed: false,
            statements: Vec::new(),
        }
    }

    pub fn scan_reachability(
        &self,
        cve_id: &str,
        package: &str,
        vuln_symbol: &str,
        source_content: &str,
    ) -> OpenVexReport {
        let stmt =
            self.pruner
                .evaluate_cve_reachability(cve_id, package, vuln_symbol, source_content);
        let passed = matches!(stmt.status, VexImpactStatus::NotAffected { .. });

        OpenVexReport {
            status: if passed {
                GateStatus::Passed
            } else {
                GateStatus::Warning(
                    "OpenVEX scanner identified reachable upstream CVE symbol in binary."
                        .to_string(),
                )
            },
            passed,
            statements: vec![stmt],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vex_scanner_nominal() {
        let scanner = OpenVexReachabilityScanner::new();
        let report = scanner.scan_reachability(
            "CVE-2026-9999",
            "serde",
            "serde::unsafe_leak",
            "fn main() {}",
        );
        assert!(report.passed);
    }
}

#[cfg(test)]
mod no_advisory_source_tests {
    use super::*;

    /// The review pipeline called this with `("CVE-NONE", "none",
    /// "symbol_none", ...)`. The scanner's decision is
    /// `!source_code.contains(vuln_symbol)`, and no real diff contains the
    /// literal `symbol_none`, so every PR was certified NotAffected by a CVE
    /// that does not exist. No advisory feed is ever consulted.
    #[test]
    fn absent_advisories_are_unmeasured_not_a_clean_vex_attestation() {
        let report = OpenVexReachabilityScanner::new().evaluate_without_advisory_source();

        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!report.passed, "an unscanned CVE set is not a clean one");
        assert!(
            report.statements.is_empty(),
            "no advisory was read, so no VEX statement may be attested"
        );
    }

    /// The measuring path must still flag a symbol that is present.
    #[test]
    fn a_reachable_symbol_still_fails() {
        let report = OpenVexReachabilityScanner::new().scan_reachability(
            "CVE-2024-1234",
            "openssl",
            "vulnerable_parse",
            "fn caller() { vulnerable_parse(x); }",
        );
        assert!(
            !report.passed,
            "a reachable vulnerable symbol must not pass"
        );
    }
}
