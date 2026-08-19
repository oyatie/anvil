use serde::{Deserialize, Serialize};

pub mod dag_manager;
pub use dag_manager::{StackSyncPlan, StackedBranchNode, StackedDagManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackedDiffsReport {
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

    pub fn evaluate_stack_synchronization(
        &self,
        branches: &[StackedBranchNode],
    ) -> StackedDiffsReport {
        let plan = self.manager.compute_stack_plan(branches);
        let passed = plan.atomic_merge_ready;

        StackedDiffsReport { passed, plan }
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
    }
}
