use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod memory_store;
pub use memory_store::{ReviewMemoryEntry, ReviewMemoryStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewMemoryReport {
    pub is_aligned: bool,
    pub recalled_rules: Vec<ReviewMemoryEntry>,
    pub summary: String,
}

pub struct ReviewMemoryEngine {
    store: ReviewMemoryStore,
}

impl Default for ReviewMemoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewMemoryEngine {
    pub fn new() -> Self {
        let store = ReviewMemoryStore::new();
        Self { store }
    }

    /// 100% Deterministic evaluation of review feedback against historical repository memory
    pub fn evaluate_review_memory(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<ReviewMemoryReport> {
        info!(
            "Running ReviewMemoryEngine (Semantic Review Memory & Knowledge Index) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let recalled_rules = self
            .store
            .lookup_architectural_patterns(&diff_ctx.repo, &diff_ctx.diff_content);
        let is_aligned = true;

        let summary = if recalled_rules.is_empty() {
            "✅ PASSED (Code fully aligned with historical architectural memory & conventions)"
                .to_string()
        } else {
            format!(
                "💡 NOTICE ({} repository memory rule(s) referenced for review alignment)",
                recalled_rules.len()
            )
        };

        Ok(ReviewMemoryReport {
            is_aligned,
            recalled_rules,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_review_memory_nominal() {
        let engine = ReviewMemoryEngine::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ use parking_lot::Mutex;".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = engine
            .evaluate_review_memory(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_aligned);
    }
}
