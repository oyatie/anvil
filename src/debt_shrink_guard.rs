use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DebtViolation {
    pub file_path: String,
    pub lines_added: usize,
    pub lines_deleted: usize,
    pub net_growth: isize,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtShrinkReport {
    pub is_acceptable: bool,
    pub total_debt_shrunk: usize,
    pub violations: Vec<DebtViolation>,
    pub summary: String,
}

pub struct DebtShrinkGuard;

impl DebtShrinkGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates deprecation & reorg debt shrink ratchet:
    /// Allows shrinks (net deletions), forbids adding onto deprecated/reorg targets.
    pub fn evaluate_debt_shrink(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<DebtShrinkReport> {
        info!("Running DebtShrinkGuard (Deprecation & Reorg Drain Ratchet) on {}#{}...", diff_ctx.repo, diff_ctx.pr_number);

        let mut violations = Vec::new();
        let mut total_debt_shrunk: usize = 0;

        let reorg_drain_path = repo_dir.join("governance/REORG-DRAIN.md");
        let root_drain_path = repo_dir.join("REORG-DRAIN.md");
        let reorg_drain_content = if reorg_drain_path.exists() {
            std::fs::read_to_string(&reorg_drain_path).unwrap_or_default()
        } else if root_drain_path.exists() {
            std::fs::read_to_string(&root_drain_path).unwrap_or_default()
        } else {
            String::new()
        };

        let mut current_file = String::new();
        let mut file_added = 0;
        let mut file_deleted = 0;
        let mut file_diffs: Vec<(String, usize, usize)> = Vec::new();

        for line in diff_ctx.diff_content.lines() {
            if line.starts_with("+++ b/") {
                if !current_file.is_empty() {
                    file_diffs.push((current_file.clone(), file_added, file_deleted));
                }
                current_file = line[6..].trim().to_string();
                file_added = 0;
                file_deleted = 0;
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                file_added += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                file_deleted += 1;
            }
        }
        if !current_file.is_empty() {
            file_diffs.push((current_file, file_added, file_deleted));
        }

        for (file, added, deleted) in file_diffs {
            let is_deprecating = file.contains("deprecated")
                || file.contains("legacy")
                || file.contains("/old/")
                || reorg_drain_content.contains(&file);

            if is_deprecating {
                let net_growth = (added as isize) - (deleted as isize);
                if net_growth > 0 {
                    violations.push(DebtViolation {
                        file_path: file.clone(),
                        lines_added: added,
                        lines_deleted: deleted,
                        net_growth,
                        reason: "Adding onto deprecating or reorganization target is prohibited; only net shrinkage / deletions permitted.".to_string(),
                    });
                } else if net_growth < 0 {
                    total_debt_shrunk += deleted - added;
                    info!("Debt reduced on deprecating target `{}`: -{} lines", file, deleted - added);
                }
            }
        }

        let is_acceptable = violations.is_empty();
        let summary = if is_acceptable {
            if total_debt_shrunk > 0 {
                format!("Deprecation debt reduced by {} lines across reorg targets. Zero expansions permitted.", total_debt_shrunk)
            } else {
                "Deprecation & Reorg Drain Ratchet verified; zero prohibited expansions on deprecating targets.".to_string()
            }
        } else {
            format!(
                "Debt ratchet violations ({} files): {}",
                violations.len(),
                violations.iter().map(|v| format!("{} (+{} lines net)", v.file_path, v.net_growth)).collect::<Vec<_>>().join("; ")
            )
        };

        Ok(DebtShrinkReport {
            is_acceptable,
            total_debt_shrunk,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocks_addition_to_legacy_file() {
        let guard = DebtShrinkGuard::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 301,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/legacy/old_router.ts\n+ export const newFeature = true;\n+ export const moreCode = 42;".to_string(),
            changed_files: vec!["src/legacy/old_router.ts".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_debt_shrink(&temp_dir, &diff_ctx).expect("Evaluates");
        assert!(!report.is_acceptable);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].net_growth, 2);
    }

    #[test]
    fn test_allows_shrink_on_legacy_file() {
        let guard = DebtShrinkGuard::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 302,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/legacy/old_router.ts\n- export const deadCode = 1;\n- export const oldHandler = 2;".to_string(),
            changed_files: vec!["src/legacy/old_router.ts".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_debt_shrink(&temp_dir, &diff_ctx).expect("Evaluates");
        assert!(report.is_acceptable);
        assert_eq!(report.total_debt_shrunk, 2);
    }
}
