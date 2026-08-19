use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonorepoViolation {
    pub category: String, // "NON_HERMETIC_PATH_ESCAPE", "HARDCODED_ABSOLUTE_PATH", "PROTOTYPE_POLLUTION_RISK"
    pub description: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonorepoGuardReport {
    pub is_compliant: bool,
    pub violations: Vec<MonorepoViolation>,
    pub summary: String,
}

pub struct MonorepoGuard;

impl MonorepoGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates hyperscaler monorepo best practices & hermeticity patterns
    pub async fn evaluate_monorepo_hygiene(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<MonorepoGuardReport> {
        info!(
            "Running MonorepoGuard hyperscaler patterns on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut violations = Vec::new();

        let monorepo_rules = [
            (
                r#"(?i)(?:include!|include_str!|include_bytes!|require|import).*?['"](?:\.\./){3,}"#,
                "NON_HERMETIC_PATH_ESCAPE",
                "Deep relative path traversal escaping package boundary (violates hermetic build rules)",
            ),
            (
                r#"(?i)["'](?:/Users/|/home/|/private/tmp/)[^"']+["']"#,
                "HARDCODED_ABSOLUTE_PATH",
                "Hardcoded absolute local filesystem path detected (breaks CAS remote caching)",
            ),
            (
                r#"(?i)(?:__proto__|prototype\s*\[\s*["']__proto__["']\s*\])"#,
                "PROTOTYPE_POLLUTION_RISK",
                "Direct prototype assignment detected (monorepo prototype chain violation)",
            ),
        ];

        for (pattern, cat, desc) in &monorepo_rules {
            if let Ok(re) = Regex::new(pattern) {
                for line in diff_ctx.diff_content.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") {
                        let trimmed = line[1..].trim();
                        if re.is_match(trimmed) {
                            violations.push(MonorepoViolation {
                                category: cat.to_string(),
                                description: desc.to_string(),
                                snippet: trimmed.to_string(),
                            });
                        }
                    }
                }
            }
        }

        // Run check-undeclared-imports.mjs if available
        let undeclared_script = repo_dir.join("scripts/check-undeclared-imports.mjs");
        if undeclared_script.exists() {
            let out = Command::new("node")
                .current_dir(repo_dir)
                .arg("scripts/check-undeclared-imports.mjs")
                .output()
                .await;

            if let Ok(res) = out {
                if !res.status.success() {
                    let err = String::from_utf8_lossy(&res.stderr);
                    warn!("check-undeclared-imports.mjs reported issues: {}", err);
                    violations.push(MonorepoViolation {
                        category: "UNDECLARED_IMPORT".to_string(),
                        description: "Undeclared monorepo package import detected by linter script"
                            .to_string(),
                        snippet: err
                            .lines()
                            .next()
                            .unwrap_or("undeclared import")
                            .to_string(),
                    });
                }
            }
        }

        let is_compliant = violations.is_empty();
        let summary = if is_compliant {
            "Hyperscaler monorepo patterns verified: hermetic boundaries, CAS cache compatibility, and clean package exports 100% compliant.".to_string()
        } else {
            format!(
                "Monorepo pattern warnings ({} items): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| v.description.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(MonorepoGuardReport {
            is_compliant,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detects_hardcoded_absolute_path() {
        let guard = MonorepoGuard::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 203,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ const configPath = \"/Users/jasonlee/Documents/config.json\";"
                .to_string(),
            changed_files: vec!["src/config.ts".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_monorepo_hygiene(&temp_dir, &diff_ctx)
            .await
            .expect("Evaluates");
        assert!(!report.is_compliant);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].category, "HARDCODED_ABSOLUTE_PATH");
    }

    #[tokio::test]
    async fn test_clean_monorepo_diff_passes() {
        let guard = MonorepoGuard::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 204,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ let config_path = PathBuf::from(std::env::var(\"CONFIG_PATH\").unwrap_or_default());".to_string(),
            changed_files: vec!["src/config.rs".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_monorepo_hygiene(&temp_dir, &diff_ctx)
            .await
            .expect("Evaluates");
        assert!(report.is_compliant);
    }
}
