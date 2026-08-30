use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod phase_validator;
pub use phase_validator::{MigrationPhaseFinding, MigrationPhaseValidator};

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "migration_orch_status";

const NO_SQL_IN_SCOPE: &str = "no file in this diff is a `.sql` migration, so no schema transition was parsed and no \
     Expand-Contract phase order was checked; an empty scope is not an ordered lifecycle";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationLifecycleReport {
    pub status: GateStatus,
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
        let mut parsed_a_migration = false;

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            // A chunk with no `diff --git` header names no file, and a chunk
            // that names no file is not a migration. It used to default to
            // `migration.sql`, which put the split's empty leading chunk in
            // scope on every diff.
            let Some(current_file) = file_diff
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().last())
                .map(|p| p.trim_start_matches("b/").to_string())
            else {
                continue;
            };

            if !MigrationPhaseValidator::is_migration_sql(&current_file) {
                continue;
            }
            parsed_a_migration = true;

            let file_findings = self
                .validator
                .validate_migration_sql(&current_file, file_diff);
            findings.extend(file_findings);
        }

        // Nothing in scope is not the same as nothing wrong. The scope is one
        // file extension, and the sibling gate that judges the same subject
        // (`ghost_migration_status`) uses a wider one, so the two disagree
        // about what a migration is. A pass here would certify phase ordering
        // for a schema this gate never read.
        if !parsed_a_migration {
            return Ok(MigrationLifecycleReport {
                status: GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason: NO_SQL_IN_SCOPE.to_string(),
                },
                is_ordered: false,
                findings,
                summary: NO_SQL_IN_SCOPE.to_string(),
            });
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
            status: if is_ordered {
                GateStatus::Passed
            } else {
                GateStatus::Failed(summary.clone())
            },
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
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("."),
                crate::git_manager::Uncloned::TestFixture,
            ),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = orch
            .evaluate_migration_lifecycle(Path::new("."), &diff_ctx)
            .unwrap();

        // This asserted `rep.is_ordered` for a diff carrying no `diff --git`
        // header at all: the old scope defaulted the filename to
        // `migration.sql` and then found no `.sql` in the hunk text, so the
        // chunk was skipped and the gate certified an empty scan. Out of scope
        // is now unmeasured.
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!rep.is_ordered);
    }
}
