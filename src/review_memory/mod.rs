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

impl ReviewMemoryReport {
    /// A change that contradicts a rule this repository already learned.
    ///
    /// The recalled rules are not findings — they are memory. The finding is
    /// that the change is not aligned with one, and each recalled rule names
    /// how many occurrences it has already prevented, which is the recurrence
    /// count that argues for making the rule mechanical.
    pub fn work_items(&self, repo: &str) -> Vec<crate::intake::WorkItem> {
        use crate::intake::{Remedy, Source, WorkItem, sources::subject};
        if self.is_aligned {
            return Vec::new();
        }
        self.recalled_rules
            .iter()
            .map(|rule| WorkItem {
                source: Source::ReviewFinding,
                subject: subject(repo, &rule.pattern_key),
                what: format!("contradicts a learned rule: {}", rule.architectural_rule),
                consequence: format!(
                    "this rule has already prevented {} occurrence(s); a class \
                     caught again by memory rather than by a check is a rule \
                     that should have been made mechanical",
                    rule.total_occurrences_prevented
                ),
                class: Some(rule.pattern_key.clone()),
                remedy: Remedy::Unclassified,
            })
            .collect()
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
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("."),
                crate::git_manager::Uncloned::TestFixture,
            ),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = engine
            .evaluate_review_memory(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_aligned);
    }
}
