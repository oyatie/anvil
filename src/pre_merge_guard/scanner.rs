use super::GateStatus;
use crate::harness::judgement;
use regex::Regex;

pub struct PreMergeScanner;

impl PreMergeScanner {
    /// Shannon entropy of `s` in bits per character.
    ///
    /// Moved to [`crate::harness::judgement`] and delegated here. This gate is
    /// `Superseded`; the judgement is not, and a `Migrating` rule may not be
    /// anchored to a module scheduled for deletion.
    pub fn shannon_entropy(s: &str) -> f64 {
        judgement::shannon_entropy(s)
    }

    /// Whether a structurally matched candidate survives the false-positive
    /// filters. See [`crate::harness::judgement::is_credential_shaped`].
    pub fn is_credential_shaped(candidate: &str, min_entropy: f64) -> bool {
        judgement::is_credential_shaped(candidate, min_entropy)
    }

    /// Scans the lines a diff ADDS for a credential.
    /// See [`crate::harness::judgement::scan_for_secrets`].
    pub fn scan_for_secrets(diff: &str) -> GateStatus {
        judgement::scan_for_secrets(diff)
    }

    pub fn scan_for_breaking_changes(diff: &str, changed_files: &[String]) -> GateStatus {
        let has_migration = changed_files
            .iter()
            .any(|f| f.contains("migration") || f.ends_with(".sql"));

        if has_migration {
            let destructive_patterns = [
                r"(?i)DROP\s+COLUMN",
                r"(?i)DROP\s+TABLE",
                r"(?i)ALTER\s+COLUMN.*NOT\s+NULL",
            ];

            for pattern in destructive_patterns {
                if let Ok(re) = Regex::new(pattern) {
                    for line in diff.lines() {
                        if line.starts_with('+') && !line.starts_with("+++") && re.is_match(line) {
                            return GateStatus::Warning(
                                    "Destructive schema migration detected (DROP/NOT NULL without multi-phase rollout). Verify backwards compatibility across cell nodes.".to_string(),
                                );
                        }
                    }
                }
            }
        }

        GateStatus::Passed
    }

    pub fn scan_for_concurrency_and_flakes(diff: &str) -> GateStatus {
        let flake_patterns = [
            (
                r"(?i)thread::sleep\s*\(\s*Duration::from_millis\s*\(\s*\d+\s*\)\s*\)",
                "Hardcoded real-clock test sleep (risk of test lane flake)",
            ),
            (
                r"(?i)time\.Sleep\s*\(\s*\d+\s*\*\s*time\.Millisecond\s*\)",
                "Hardcoded real-clock test sleep",
            ),
        ];

        for (pattern, desc) in flake_patterns {
            if let Ok(re) = Regex::new(pattern) {
                for line in diff.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") && re.is_match(line) {
                        return GateStatus::Warning(format!(
                            "Concurrency/Timing Warning: {}",
                            desc
                        ));
                    }
                }
            }
        }

        GateStatus::Passed
    }
}
