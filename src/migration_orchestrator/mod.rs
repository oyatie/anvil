use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod phase_validator;
pub use phase_validator::{MigrationPhaseFinding, MigrationPhaseValidator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationLifecycleReport {
    pub is_ordered: bool,
    pub findings: Vec<MigrationPhaseFinding>,
    pub summary: String,
}

pub struct MigrationLifecycleOrchestrator {
    validator: MigrationPhaseValidator,
}

impl Default for MigrationLifecycleOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl MigrationLifecycleOrchestrator {
    pub fn new() -> Self {
        let validator = MigrationPhaseValidator::new();
        Self { validator }
    }

    /// 100% Deterministic evaluation of multi-phase Expand-Contract database schema migrations
    pub fn evaluate_migration_lifecycle(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<MigrationLifecycleReport> {
        info!(
            "Running MigrationLifecycleOrchestrator (Deterministic Expand-Contract Lifecycle) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings = Vec::new();

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            if !file_diff.contains(".sql") {
                continue;
            }

            let lines: Vec<&str> = file_diff.lines().collect();
            let mut current_file = "migration.sql".to_string();
            if let Some(first_line) = lines.first()
                && let Some(path) = first_line.split_whitespace().last()
            {
                current_file = path.trim_start_matches("b/").to_string();
            }

            let file_findings = self
                .validator
                .validate_migration_sql(&current_file, file_diff);
            findings.extend(file_findings);
        }

        let is_ordered = findings.is_empty();
        let summary = if is_ordered {
            "✅ PASSED (All database schema transitions adhere to Expand-Contract lifecycle invariants)".to_string()
        } else {
            format!(
                "❌ FAILED ({} database migration phase order violation(s) detected)",
                findings.len()
            )
        };

        Ok(MigrationLifecycleReport {
            is_ordered,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_orchestrator_nominal() {
        let orch = MigrationLifecycleOrchestrator::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ ALTER TABLE records ADD COLUMN tenant_id UUID;".to_string(),
            changed_files: vec!["migrations/0010_add_tenant.sql".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = orch
            .evaluate_migration_lifecycle(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_ordered);
    }
}
