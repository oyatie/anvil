use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod outbox_rules;
pub use outbox_rules::{IdempotencyFinding, OutboxRulesEngine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyReport {
    pub is_idempotent: bool,
    pub findings: Vec<IdempotencyFinding>,
    pub summary: String,
}

pub struct IdempotencyGuard {
    engine: OutboxRulesEngine,
}

impl IdempotencyGuard {
    pub fn new() -> Self {
        let engine = OutboxRulesEngine::new();
        Self { engine }
    }

    /// Evaluates PR diffs for state-mutation idempotency and transactional outbox patterns
    pub fn evaluate_idempotency(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<IdempotencyReport> {
        info!(
            "Running IdempotencyGuard (Stripe Idempotency Keys & Transactional Outbox) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings = Vec::new();

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            if !file_diff.contains(".rs") {
                continue;
            }

            let lines: Vec<&str> = file_diff.lines().collect();
            let mut current_file = "unknown.rs".to_string();
            if let Some(first_line) = lines.first() {
                if let Some(path) = first_line.split_whitespace().last() {
                    current_file = path.trim_start_matches("b/").to_string();
                }
            }

            let file_findings = self.engine.scan_mutating_endpoints(&current_file, file_diff);
            findings.extend(file_findings);
        }

        let is_idempotent = findings.is_empty();
        let summary = if is_idempotent {
            "✅ PASSED (All state-mutating endpoints enforce Idempotency-Key headers and outbox safety)".to_string()
        } else {
            format!(
                "⚠️ CAUTION ({} mutating endpoint(s) lack Idempotency-Key validation)",
                findings.len()
            )
        };

        Ok(IdempotencyReport {
            is_idempotent,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idempotency_guard_nominal() {
        let guard = IdempotencyGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ pub fn get_status() -> bool { true }".to_string(),
            changed_files: vec!["src/status.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard.evaluate_idempotency(Path::new("."), &diff_ctx).unwrap();
        assert!(rep.is_idempotent);
    }
}
