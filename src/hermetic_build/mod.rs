use serde::{Deserialize, Serialize};

pub mod reproducibility_checker;
pub use reproducibility_checker::{ReproducibilityChecker, ReproducibilityResult};

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "hermetic_build_status";

const MISSING_BUILD_PAIR: &str = "no second build was produced, so bit-for-bit reproducibility was \
     never compared for this pull request";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermeticBuildReport {
    pub status: GateStatus,
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

    /// The gate's answer when no pair of builds exists to compare.
    ///
    /// The pipeline passed the literal `"sha256_clean"` as both digests, so
    /// the equality check compared a string to itself.
    pub fn evaluate_without_build_pair(&self) -> HermeticBuildReport {
        HermeticBuildReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_BUILD_PAIR.to_string(),
            },
            passed: false,
            result: ReproducibilityResult::NonDeterministic {
                build_a_sha256: String::new(),
                build_b_sha256: String::new(),
                divergent_symbol_or_path: MISSING_BUILD_PAIR.to_string(),
            },
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

        HermeticBuildReport {
            status: if passed {
                GateStatus::Passed
            } else {
                GateStatus::Failed(
                    "Hermetic build reproducibility check detected bitwise artifact non-determinism."
                        .to_string(),
                )
            },
            passed,
            result,
        }
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

#[cfg(test)]
mod no_build_pair_tests {
    use super::*;

    /// The review pipeline called this with the literal `"sha256_clean"` as
    /// BOTH build digests, so `build_a_hash == build_b_hash` compared a string
    /// to itself and was true on every pull request. No second build was ever
    /// produced, and the only route to a failure was an unrelated substring
    /// check for `SystemTime::now()` in the diff.
    #[test]
    fn absent_build_digests_are_unmeasured_not_bit_for_bit_identical() {
        let report = HermeticBuildValidator::new().evaluate_without_build_pair();

        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(
            !report.passed,
            "a binary that was never built twice is not reproducible"
        );
    }

    /// The measuring path must still detect divergence between two real builds.
    #[test]
    fn two_divergent_digests_still_fail() {
        let report = HermeticBuildValidator::new().evaluate_hermetic_reproducibility(
            "sha256_aaa",
            "sha256_bbb",
            "",
        );
        assert!(!report.passed, "mismatched digests must not pass");
    }
}
