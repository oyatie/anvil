use serde::{Deserialize, Serialize};

pub mod callgraph_pruner;
pub use callgraph_pruner::{CallgraphPruner, OpenVexStatement, VexImpactStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenVexReport {
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

    pub fn scan_reachability(
        &self,
        cve_id: &str,
        package: &str,
        vuln_symbol: &str,
        source_content: &str,
    ) -> OpenVexReport {
        let stmt = self.pruner.evaluate_cve_reachability(cve_id, package, vuln_symbol, source_content);
        let passed = matches!(stmt.status, VexImpactStatus::NotAffected { .. });

        OpenVexReport {
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
        let report = scanner.scan_reachability("CVE-2026-9999", "serde", "serde::unsafe_leak", "fn main() {}");
        assert!(report.passed);
    }
}
