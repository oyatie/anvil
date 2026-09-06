use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "debt_shrink_status";

const NO_DEPRECATING_TARGET_IN_SCOPE: &str = "no changed file is a deprecating target -- none matched the marker set (`deprecated`, \
     `legacy`, `/old/`) and no REORG-DRAIN.md drain ledger named one -- so no debt was measured; \
     an empty scope is not a drained one";

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
    pub status: GateStatus,
    pub is_acceptable: bool,
    pub total_debt_shrunk: usize,
    pub violations: Vec<DebtViolation>,
    pub summary: String,
}

pub struct DebtShrinkGuard;

impl Default for DebtShrinkGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl DebtShrinkGuard {
    pub fn new() -> Self {
        Self
    }

    /// Whether a changed path is a deprecating or reorganisation target -- the
    /// scope this ratchet inspects.
    ///
    /// `pub` because the caller must distinguish "scanned and clean" from
    /// "nothing was in scope"; the predicate was inline and unreachable.
    ///
    /// The marker half is a guess about spelling, and the drain ledger is the
    /// only authoritative half: deprecation is a decision somebody recorded,
    /// not a substring somebody typed into a path. With no ledger present, a
    /// path that does not spell one of three fragments is invisible here.
    pub fn is_deprecating_target(file_path: &str, drain_ledger: &str) -> bool {
        file_path.contains("deprecated")
            || file_path.contains("legacy")
            || file_path.contains("/old/")
            || (!drain_ledger.is_empty() && drain_ledger.contains(file_path))
    }

    /// Evaluates deprecation & reorg debt shrink ratchet:
    /// Allows shrinks (net deletions), forbids adding onto deprecated/reorg targets.
    pub fn evaluate_debt_shrink(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<DebtShrinkReport> {
        info!(
            "Running DebtShrinkGuard (Deprecation & Reorg Drain Ratchet) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

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
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                if !current_file.is_empty() {
                    file_diffs.push((current_file.clone(), file_added, file_deleted));
                }
                current_file = stripped.trim().to_string();
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

        let mut scanned_a_target = false;

        for (file, added, deleted) in file_diffs {
            let is_deprecating = Self::is_deprecating_target(&file, &reorg_drain_content);

            if is_deprecating {
                scanned_a_target = true;
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
                    info!(
                        "Debt reduced on deprecating target `{}`: -{} lines",
                        file,
                        deleted - added
                    );
                }
            }
        }

        // Nothing in scope is not the same as nothing wrong. The marker set is
        // three path fragments and the drain ledger is absent from this
        // repository, so a repository that spells deprecation any other way can
        // never put a file in scope -- and a pass here would certify a debt
        // ratchet against a corpus the guard never had.
        if !scanned_a_target {
            return Ok(DebtShrinkReport {
                status: GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason: NO_DEPRECATING_TARGET_IN_SCOPE.to_string(),
                },
                is_acceptable: false,
                total_debt_shrunk: 0,
                violations,
                summary: NO_DEPRECATING_TARGET_IN_SCOPE.to_string(),
            });
        }

        let is_acceptable = violations.is_empty();
        let summary = if is_acceptable {
            if total_debt_shrunk > 0 {
                format!(
                    "Deprecation debt reduced by {} lines across reorg targets. Zero expansions permitted.",
                    total_debt_shrunk
                )
            } else {
                "Deprecation & Reorg Drain Ratchet verified; zero prohibited expansions on deprecating targets.".to_string()
            }
        } else {
            format!(
                "Debt ratchet violations ({} files): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{} (+{} lines net)", v.file_path, v.net_growth))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(DebtShrinkReport {
            status: if is_acceptable {
                GateStatus::Passed
            } else {
                GateStatus::Failed(summary.clone())
            },
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
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(std::path::PathBuf::from("/tmp"), crate::git_manager::Uncloned::TestFixture),
            diff_content: "+++ b/src/legacy/old_router.ts\n+ export const newFeature = true;\n+ export const moreCode = 42;".to_string(),
            changed_files: vec!["src/legacy/old_router.ts".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_debt_shrink(&temp_dir, &diff_ctx)
            .expect("Evaluates");
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
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(std::path::PathBuf::from("/tmp"), crate::git_manager::Uncloned::TestFixture),
            diff_content: "+++ b/src/legacy/old_router.ts\n- export const deadCode = 1;\n- export const oldHandler = 2;".to_string(),
            changed_files: vec!["src/legacy/old_router.ts".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_debt_shrink(&temp_dir, &diff_ctx)
            .expect("Evaluates");
        assert!(report.is_acceptable);
        assert_eq!(report.total_debt_shrunk, 2);
    }
}
