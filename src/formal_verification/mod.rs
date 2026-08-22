use serde::{Deserialize, Serialize};

pub mod policy_scanner;
pub use policy_scanner::{PolicyPatternScanner, PolicyScanResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormalVerificationFinding {
    pub rule: String,
    pub matched_text: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormalVerificationReport {
    pub passed: bool,
    pub findings: Vec<FormalVerificationFinding>,
}

#[derive(Debug, Clone, Default)]
pub struct FormalVerificationGuard {
    solver: PolicyPatternScanner,
}

impl FormalVerificationGuard {
    pub fn new() -> Self {
        Self {
            solver: PolicyPatternScanner::new(),
        }
    }

    pub fn evaluate_formal_invariants(&self, diff_content: &str) -> FormalVerificationReport {
        let mut findings = Vec::new();

        match self.solver.scan_policy_text(diff_content) {
            PolicyScanResult::PatternMatched {
                rule_name,
                matched_text,
                explanation,
            } => {
                findings.push(FormalVerificationFinding {
                    rule: rule_name,
                    matched_text,
                    message: explanation,
                });
            }
            PolicyScanResult::NoPatternMatched => {}
        }

        FormalVerificationReport {
            passed: findings.is_empty(),
            findings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formal_verification_nominal() {
        let guard = FormalVerificationGuard::new();
        let report = guard.evaluate_formal_invariants("let x = 42;");
        assert!(report.passed);
        assert!(report.findings.is_empty());
    }
}
