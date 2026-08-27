use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

pub mod disposition;
pub mod harness_quarantine;
pub mod whole_file_expansion;
pub use disposition::{
    ComponentDisposition, ComponentDispositionClassifier, ComponentEvaluationReport,
};
pub use harness_quarantine::HarnessQuarantine;
pub use whole_file_expansion::WholeFileExpansion;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonorepoViolation {
    pub category: String, // "AI_HARNESS_COMMIT_LEAK", "UNAUTHORIZED_AUTHORITY_CLAIM", "NON_HERMETIC_PATH_ESCAPE", "HARDCODED_ABSOLUTE_PATH"
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

impl Default for MonorepoGuard {
    fn default() -> Self {
        Self::new()
    }
}

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
            "Running monorepo boundary rules (hermetic boundaries, harness quarantine, SSOT authority) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut violations = Vec::new();

        // 1. Check for AI / LLM Agent Scratch Harness Commit Leaks
        let harness_violations =
            HarnessQuarantine::check_harness_quarantine(&diff_ctx.changed_files);
        violations.extend(harness_violations);

        // 2. Check for Unauthorized SSOT Authority Claims
        // The callee's parameter is named `file_content`; this passed the whole
        // diff. One file claiming canonical authority therefore accused every
        // non-canonical path in the change, each by name. The parameter name
        // stated the contract correctly and only the call site broke it.
        for fd in crate::git_manager::diff_context::diffs_by_path(&diff_ctx.diff_content) {
            if let Some(v) =
                HarnessQuarantine::check_ssot_authority_location(&fd.path, fd.after_change())
            {
                violations.push(v);
            }
        }

        // 3. Whole-File Context Expansion: Evaluate entire touched files on disk
        for file in &diff_ctx.changed_files {
            let whole_file_violations = WholeFileExpansion::evaluate_whole_file(repo_dir, file);
            violations.extend(whole_file_violations);
        }

        // 4. Check for Non-Hermetic Path Escapes & Hardcoded Absolute Paths
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

        // Run check-undeclared-imports.mjs if available.
        // Fails closed: a linter that hangs, is missing or crashes yields a
        // violation, never a silent "no undeclared imports".
        let undeclared_script = repo_dir.join("scripts/check-undeclared-imports.mjs");
        if undeclared_script.exists() {
            let mut cmd = Command::new("node");
            cmd.current_dir(repo_dir)
                .arg("scripts/check-undeclared-imports.mjs");

            match crate::exec::run_bounded(
                cmd,
                crate::exec::ExecClass::Build,
                "check-undeclared-imports.mjs",
            )
            .await
            {
                Ok(res) => {
                    if !res.status.success() {
                        let err = String::from_utf8_lossy(&res.stderr);
                        warn!("check-undeclared-imports.mjs reported issues: {}", err);
                        violations.push(MonorepoViolation {
                            category: "UNDECLARED_IMPORT".to_string(),
                            description:
                                "Undeclared monorepo package import detected by linter script"
                                    .to_string(),
                            snippet: err
                                .lines()
                                .next()
                                .unwrap_or("undeclared import")
                                .to_string(),
                        });
                    }
                }
                Err(e) => {
                    warn!(
                        "check-undeclared-imports.mjs did not complete ({}). Recording a violation \
                         rather than a pass.",
                        e
                    );
                    violations.push(MonorepoViolation {
                        category: "UNDECLARED_IMPORT_CHECK_FAILED".to_string(),
                        description:
                            "Undeclared-import linter could not be run to completion, so imports \
                             are unverified"
                                .to_string(),
                        snippet: format!("check-undeclared-imports.mjs did not complete: {e}"),
                    });
                }
            }
        }

        let is_compliant = violations.is_empty();
        let summary = if is_compliant {
            "Monorepo boundary rules verified: hermetic boundaries, harness quarantine, and SSOT authority rules 100% compliant.".to_string()
        } else {
            format!(
                "Monorepo pattern warnings ({} items): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{}: {}", v.category, v.snippet))
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
    async fn test_clean_monorepo_diff_passes() {
        let guard = MonorepoGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 101,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ use crate::types::Model;".to_string(),
            changed_files: vec!["crates/core/src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            is_incremental: false,
            previous_head_sha: None,
        };

        let report = guard
            .evaluate_monorepo_hygiene(Path::new("/tmp"), &diff_ctx)
            .await
            .unwrap();
        assert!(report.is_compliant);
    }

    #[tokio::test]
    async fn test_detects_hardcoded_absolute_path() {
        let guard = MonorepoGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 102,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: r#"+ let config_path = "/Users/developer/app.json";"#.to_string(),
            changed_files: vec!["crates/core/src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            is_incremental: false,
            previous_head_sha: None,
        };

        let report = guard
            .evaluate_monorepo_hygiene(Path::new("/tmp"), &diff_ctx)
            .await
            .unwrap();
        assert!(!report.is_compliant);
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.category == "HARDCODED_ABSOLUTE_PATH")
        );
    }
}
