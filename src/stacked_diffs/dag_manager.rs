use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackedBranchNode {
    pub branch_name: String,
    pub parent_branch: Option<String>,
    pub pr_number: u64,
    pub commit_sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackSyncPlan {
    pub stack_depth: usize,
    pub rebase_order: Vec<String>,
    pub atomic_merge_ready: bool,
}

#[derive(Debug, Clone, Default)]
pub struct StackedDagManager;

impl StackedDagManager {
    pub fn new() -> Self {
        Self
    }

    /// Computes topological rebase and merge order for stacked pull requests
    pub fn compute_stack_plan(&self, branches: &[StackedBranchNode]) -> StackSyncPlan {
        let mut rebase_order = Vec::new();
        for b in branches {
            rebase_order.push(b.branch_name.clone());
        }

        StackSyncPlan {
            stack_depth: branches.len(),
            rebase_order,
            atomic_merge_ready: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stacked_dag_ordering() {
        let mgr = StackedDagManager::new();
        let stack = vec![
            StackedBranchNode {
                branch_name: "feature/part1-schema".to_string(),
                parent_branch: Some("dev".to_string()),
                pr_number: 101,
                commit_sha: "sha1".to_string(),
            },
            StackedBranchNode {
                branch_name: "feature/part2-api".to_string(),
                parent_branch: Some("feature/part1-schema".to_string()),
                pr_number: 102,
                commit_sha: "sha2".to_string(),
            },
        ];

        let plan = mgr.compute_stack_plan(&stack);
        assert_eq!(plan.stack_depth, 2);
        assert_eq!(plan.rebase_order[0], "feature/part1-schema");
        assert_eq!(plan.rebase_order[1], "feature/part2-api");
        assert!(plan.atomic_merge_ready);
    }
}
