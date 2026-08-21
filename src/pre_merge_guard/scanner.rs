use super::GateStatus;
use regex::Regex;

pub struct PreMergeScanner;

impl PreMergeScanner {
    pub fn scan_for_secrets(diff: &str) -> GateStatus {
        let secret_patterns = [
            (
                r"(?i)-----BEGIN[ A-Z0-9_-]*PRIVATE KEY-----",
                "Exposed Private Key block",
            ),
            (r"(?i)AKIA[0-9A-Z]{16}", "AWS Access Key ID"),
            (r"(?i)ghp_[A-Za-z0-9_]{36}", "GitHub Personal Access Token"),
            (r"(?i)gho_[A-Za-z0-9_]{36}", "GitHub OAuth Token"),
            (r"(?i)sk-[A-Za-z0-9_-]{24,}", "API Secret Key"),
            (
                r#"(?i)password\s*[:=]\s*["'][^"']{6,}["']"#,
                "Hardcoded plaintext password",
            ),
        ];

        for (pattern, desc) in secret_patterns {
            if let Ok(re) = Regex::new(pattern) {
                for line in diff.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") && re.is_match(line) {
                        return GateStatus::Failed(format!("Potential credential leak: {}", desc));
                    }
                }
            }
        }

        GateStatus::Passed
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
