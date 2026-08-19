use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReproducibilityResult {
    DeterministicBitForBit,
    NonDeterministic {
        build_a_sha256: String,
        build_b_sha256: String,
        divergent_symbol_or_path: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct ReproducibilityChecker;

impl ReproducibilityChecker {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates build artifact checksums and flags embedded timestamp or ambient filesystem leaks
    pub fn check_build_artifacts(
        &self,
        build_a_hash: &str,
        build_b_hash: &str,
        source_diff: &str,
    ) -> ReproducibilityResult {
        // Flag non-deterministic patterns in build scripts (e.g. `std::time::SystemTime::now()` or `env!("PWD")`)
        if source_diff.contains("SystemTime::now()") || source_diff.contains("env!(\"HOME\")") {
            return ReproducibilityResult::NonDeterministic {
                build_a_sha256: build_a_hash.to_string(),
                build_b_sha256: build_b_hash.to_string(),
                divergent_symbol_or_path: "Detected ambient impurity: non-hermetic timestamp/path embedding in build artifact".to_string(),
            };
        }

        if build_a_hash == build_b_hash {
            ReproducibilityResult::DeterministicBitForBit
        } else {
            ReproducibilityResult::NonDeterministic {
                build_a_sha256: build_a_hash.to_string(),
                build_b_sha256: build_b_hash.to_string(),
                divergent_symbol_or_path: "Checksum mismatch between clean isolated sandboxes".to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_ambient_systemtime() {
        let checker = ReproducibilityChecker::new();
        let diff = r#"const BUILD_TIMESTAMP: &str = SystemTime::now();"#;
        let res = checker.check_build_artifacts("hash1", "hash1", diff);
        match res {
            ReproducibilityResult::NonDeterministic { divergent_symbol_or_path, .. } => {
                assert!(divergent_symbol_or_path.contains("timestamp"));
            }
            _ => panic!("Expected non-deterministic failure"),
        }
    }

    #[test]
    fn test_passes_deterministic_hashes() {
        let checker = ReproducibilityChecker::new();
        let res = checker.check_build_artifacts("sha256_clean", "sha256_clean", "let x = 1;");
        assert_eq!(res, ReproducibilityResult::DeterministicBitForBit);
    }
}
