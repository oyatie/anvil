use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationViolation {
    pub file_path: String,
    pub violation_type: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostMigrationReport {
    pub is_safe: bool,
    pub migrations_evaluated: usize,
    pub violations: Vec<MigrationViolation>,
    pub summary: String,
}

pub struct GhostMigrationHarness;

impl GhostMigrationHarness {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates database schema migrations for zero exclusive locks, table rewrites, and rollback parity
    pub fn evaluate_migrations(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<GhostMigrationReport> {
        info!(
            "Running GhostMigrationHarness on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut violations = Vec::new();

        let migration_files: Vec<&String> = diff_ctx
            .changed_files
            .iter()
            .filter(|f| f.contains("migration") || f.ends_with(".sql"))
            .collect();

        if migration_files.is_empty() {
            return Ok(GhostMigrationReport {
                is_safe: true,
                migrations_evaluated: 0,
                violations: Vec::new(),
                summary: "Zero database schema migrations in PR diff; ghost migration check passed.".to_string(),
            });
        }

        let migrations_evaluated = migration_files.len();

        let create_index_re = Regex::new(r"(?i)CREATE\s+(?:UNIQUE\s+)?INDEX").unwrap();
        let add_not_null_re = Regex::new(r"(?i)ADD\s+COLUMN\s+([^\s]+)\s+([^\s]+)\s+NOT\s+NULL").unwrap();
        let drop_column_re = Regex::new(r"(?i)DROP\s+COLUMN\s+([^\s]+)").unwrap();
        let drop_table_re = Regex::new(r"(?i)DROP\s+TABLE\s+([^\s]+)").unwrap();

        let mut current_file = String::new();

        for line in diff_ctx.diff_content.lines() {
            if line.starts_with("+++ b/") {
                current_file = line[6..].trim().to_string();
                continue;
            }

            if !current_file.contains("migration") && !current_file.ends_with(".sql") {
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let added_line = &line[1..].trim();
                let upper = added_line.to_uppercase();

                // 1. Non-concurrent Index Creation (Exclusive Lock Hazard)
                if create_index_re.is_match(added_line) && !upper.contains("CONCURRENTLY") {
                    violations.push(MigrationViolation {
                        file_path: current_file.clone(),
                        violation_type: "EXCLUSIVE_INDEX_LOCK".to_string(),
                        description: "Creating an index without CONCURRENTLY takes an exclusive table lock blocking all writes.".to_string(),
                        recommendation: "Use `CREATE INDEX CONCURRENTLY` to avoid production downtime.".to_string(),
                    });
                }

                // 2. Adding NOT NULL Column without DEFAULT (Full Table Rewrite & Lock)
                if add_not_null_re.is_match(added_line) && !upper.contains("DEFAULT") {
                    violations.push(MigrationViolation {
                        file_path: current_file.clone(),
                        violation_type: "TABLE_REWRITE_LOCK".to_string(),
                        description: "Adding a NOT NULL column without a server-side DEFAULT rewrites the entire table under an exclusive lock.".to_string(),
                        recommendation: "Add column as nullable, backfill asynchronously, and add NOT NULL constraint in a subsequent release.".to_string(),
                    });
                }

                // 3. Destructive DROP COLUMN without Multi-Phase Rollout
                if drop_column_re.is_match(added_line) {
                    violations.push(MigrationViolation {
                        file_path: current_file.clone(),
                        violation_type: "DESTRUCTIVE_DROP_COLUMN".to_string(),
                        description: "Immediate DROP COLUMN breaks active cell nodes still reading the old column.".to_string(),
                        recommendation: "Follow expand-contract: stop reading/writing column in app code before executing DDL drop.".to_string(),
                    });
                }

                // 4. Destructive DROP TABLE
                if drop_table_re.is_match(added_line) {
                    violations.push(MigrationViolation {
                        file_path: current_file.clone(),
                        violation_type: "DESTRUCTIVE_DROP_TABLE".to_string(),
                        description: "Immediate DROP TABLE causes instant 500 errors across rolling deployment nodes.".to_string(),
                        recommendation: "Deprecate table access across all cell pods before dropping table.".to_string(),
                    });
                }
            }
        }

        // 5. Check Rollback Parity (down.sql presence)
        for f in &migration_files {
            if f.contains("/up.sql") || f.ends_with("_up.sql") {
                let down_equivalent = f.replace("up.sql", "down.sql");
                let has_down = diff_ctx.changed_files.iter().any(|cf| cf == &down_equivalent);
                if !has_down {
                    violations.push(MigrationViolation {
                        file_path: (*f).clone(),
                        violation_type: "MISSING_DOWN_MIGRATION".to_string(),
                        description: "Forward migration added without corresponding rollback script (`down.sql`).".to_string(),
                        recommendation: format!("Provide matching rollback script at `{}`.", down_equivalent),
                    });
                }
            }
        }

        let is_safe = violations.is_empty();
        let summary = if is_safe {
            format!(
                "Ghost migration verified: {} migration(s) evaluated with zero exclusive locks or table rewrites.",
                migrations_evaluated
            )
        } else {
            format!(
                "Ghost migration hazards detected ({} violations across {} files): {}",
                violations.len(),
                migrations_evaluated,
                violations
                    .iter()
                    .map(|v| format!("{}: {}", v.file_path, v.violation_type))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(GhostMigrationReport {
            is_safe,
            migrations_evaluated,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrent_index_passes() {
        let harness = GhostMigrationHarness::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 101,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/migrations/001_add_idx.sql\n+ CREATE INDEX CONCURRENTLY idx_users_tenant ON users(tenant_id);".to_string(),
            changed_files: vec!["migrations/001_add_idx.sql".to_string()],
            is_incremental: false,
        };

        let report = harness.evaluate_migrations(&temp_dir, &diff_ctx).expect("eval");
        assert!(report.is_safe);
        assert_eq!(report.migrations_evaluated, 1);
    }

    #[test]
    fn test_non_concurrent_index_fails() {
        let harness = GhostMigrationHarness::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 102,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/migrations/002_bad_idx.sql\n+ CREATE INDEX idx_users_email ON users(email);".to_string(),
            changed_files: vec!["migrations/002_bad_idx.sql".to_string()],
            is_incremental: false,
        };

        let report = harness.evaluate_migrations(&temp_dir, &diff_ctx).expect("eval");
        assert!(!report.is_safe);
        assert_eq!(report.violations[0].violation_type, "EXCLUSIVE_INDEX_LOCK");
    }
}
