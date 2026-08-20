//! Stacked diffs / PR DAG synchronization — the gate that was handed an empty
//! stack on every pull request.
//!
//! # What was here
//!
//! `StackedDiffsReport` carried a single `passed: bool`, and the review
//! pipeline called `evaluate_stack_synchronization` with an empty slice
//! literal. The slice was empty on every pull request forever, so no stack was
//! ever examined, and the report published a pass describing nothing.
//!
//! # What is here now
//!
//! With no stack information — no parent/child branch relationships read from
//! the forge — the gate reports `GateStatus::NotMeasured` naming that missing
//! source. It does not report `Failed`: an unread DAG is not an out-of-order
//! one.
//!
//! `StackedDagManager` is retained and still exported. Given a real stack it
//! computes a real rebase order, and it is the seam a forge query plugs into.
//! Its `atomic_merge_ready` is still unconditional, which is recorded in the
//! fidelity registry rather than hidden.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::report::GateStatus;

pub mod dag_manager;
pub use dag_manager::{StackSyncPlan, StackedBranchNode, StackedDagManager};

/// Matches the `PreMergeCertificationReport` field name.
const GATE_ID: &str = "stacked_diffs_status";

/// What must exist before a stack can be evaluated at all.
const MISSING_STACK_SOURCE: &str =
    "no pull request DAG was read from the forge, so this PR's parent branch and \
     any children stacked on it are unknown and no stack was evaluated";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackedDiffsReport {
    pub status: GateStatus,
    /// Whether the computed plan is atomically mergeable. Describes the plan
    /// below, and nothing at all while `status` is `NotMeasured` — there is no
    /// stack for it to describe. Read `status`.
    pub passed: bool,
    pub plan: StackSyncPlan,
}

#[derive(Debug, Clone, Default)]
pub struct StackedDiffsOrchestrator {
    manager: StackedDagManager,
}

impl StackedDiffsOrchestrator {
    pub fn new() -> Self {
        Self {
            manager: StackedDagManager::new(),
        }
    }

    /// Computes the rebase plan for a caller-supplied stack. An empty stack is
    /// not a synchronized one: it is the absence of stack information, and is
    /// reported as such.
    pub fn evaluate_stack_synchronization(
        &self,
        branches: &[StackedBranchNode],
    ) -> StackedDiffsReport {
        let plan = self.manager.compute_stack_plan(branches);
        let passed = plan.atomic_merge_ready;

        let status = if branches.is_empty() {
            Self::not_measured()
        } else if passed {
            GateStatus::Passed
        } else {
            GateStatus::Warning(format!(
                "Stacked PR DAG of depth {} is not ready for atomic merge",
                plan.stack_depth
            ))
        };

        StackedDiffsReport {
            status,
            passed,
            plan,
        }
    }

    /// The review pipeline's entry point: no forge query is made, so no stack
    /// is invented to hand the DAG manager.
    pub fn evaluate_without_stack_source(&self) -> StackedDiffsReport {
        StackedDiffsReport {
            status: Self::not_measured(),
            passed: false,
            plan: self.manager.compute_stack_plan(&[]),
        }
    }

    fn not_measured() -> GateStatus {
        GateStatus::NotMeasured {
            gate_id: GATE_ID.to_string(),
            reason: MISSING_STACK_SOURCE.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stacked_diffs_nominal() {
        let orch = StackedDiffsOrchestrator::new();
        let rep = orch.evaluate_stack_synchronization(&[]);
        assert!(rep.passed);
        // ...and an empty stack is still not a measurement: nothing was read.
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
    }

    #[test]
    fn a_real_stack_is_measured() {
        let orch = StackedDiffsOrchestrator::new();
        let rep = orch.evaluate_stack_synchronization(&[StackedBranchNode {
            branch_name: "feature/part1-schema".to_string(),
            parent_branch: Some("dev".to_string()),
            pr_number: 101,
            commit_sha: "sha1".to_string(),
        }]);
        assert!(rep.status.is_measured());
    }

    #[test]
    fn no_forge_query_means_no_stack_claim() {
        let rep = StackedDiffsOrchestrator::new().evaluate_without_stack_source();
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!rep.passed, "absent evidence must not certify");
    }
}
