use serde::{Deserialize, Serialize};

pub mod reproducibility_checker;
pub use reproducibility_checker::{ReproducibilityChecker, ReproducibilityResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermeticBuildReport {
    pub passed: bool,
    pub result: ReproducibilityResult,
}

#[derive(Debug, Clone, Default)]
pub struct HermeticBuildValidator {
    checker: ReproducibilityChecker,
}

impl HermeticBuildValidator {
    pub fn new() -> Self {
        Self {
            checker: ReproducibilityChecker::new(),
        }
    }

    pub fn evaluate_hermetic_reproducibility(
        &self,
        build_a_hash: &str,
        build_b_hash: &str,
        diff_content: &str,
    ) -> HermeticBuildReport {
        let result = self
            .checker
            .check_build_artifacts(build_a_hash, build_b_hash, diff_content);
        let passed = matches!(result, ReproducibilityResult::DeterministicBitForBit);

        HermeticBuildReport { passed, result }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hermetic_build_nominal() {
        let val = HermeticBuildValidator::new();
        let report = val.evaluate_hermetic_reproducibility("hash1", "hash1", "const X: u32 = 42;");
        assert!(report.passed);
    }
}
