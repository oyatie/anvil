use super::GateStatus;
use crate::git_manager::SubjectRoot;

/// Maps the fallible migration source scan without converting absence into a
/// clean tree. Kept as a seam so a malformed corpus can prove the guard reports
/// `NotMeasured`, not merely that the scanner returns an error.
pub fn migration_boundary_gate_status(repo_root: &SubjectRoot) -> GateStatus {
    match crate::migration::live_tree_violations(repo_root) {
        Ok(violations) if violations.is_empty() => GateStatus::Passed,
        Ok(violations) => GateStatus::Failed(format!(
            "{} component(s) marked Migrating depend on code oyatie supersedes: {}",
            violations.len(),
            violations
                .iter()
                .map(|violation| format!("{} -> {}", violation.from, violation.to))
                .collect::<Vec<_>>()
                .join(", ")
        )),
        Err(reason) => GateStatus::NotMeasured {
            gate_id: "migration_boundary_status".to_string(),
            reason,
        },
    }
}
