use serde::{Deserialize, Serialize};

pub mod smt_solver;
pub use smt_solver::{SmtCheckResult, SmtConstraintEngine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormalVerificationFinding {
    pub rule: String,
    pub counterexample: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormalVerificationReport {
    pub passed: bool,
    pub findings: Vec<FormalVerificationFinding>,
}

#[derive(Debug, Clone, Default)]
pub struct FormalVerificationGuard {
    solver: SmtConstraintEngine,
}

impl FormalVerificationGuard {
    pub fn new() -> Self {
        Self {
            solver: SmtConstraintEngine::new(),
        }
    }

    pub fn evaluate_formal_invariants(
        &self,
        diff_content: &str,
    ) -> FormalVerificationReport {
        let mut findings = Vec::new();

        match self.solver.verify_invariants(diff_content) {
            SmtCheckResult::CounterexampleFound {
                rule_name,
                violating_tuple,
                explanation,
            } => {
                findings.push(FormalVerificationFinding {
                    rule: rule_name,
                    counterexample: violating_tuple,
                    message: explanation,
                });
            }
            SmtCheckResult::ProvablySafe => {}
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
