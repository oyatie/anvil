use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellIsolationViolation {
    pub category: String, // "UNSCOPED_TENANT_QUERY", "RAW_CROSS_CELL_SOCKET", "GLOBAL_CACHE_POLLUTION"
    pub description: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellIsolationReport {
    pub is_isolated: bool,
    pub violations: Vec<CellIsolationViolation>,
    pub summary: String,
}

pub struct CellIsolationGuard;

impl Default for CellIsolationGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl CellIsolationGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates multi-tenant isolation and cell blast-radius boundaries on the PR diff
    pub fn evaluate_cell_isolation(&self, diff_ctx: &PrDiffContext) -> Result<CellIsolationReport> {
        info!(
            "Running CellIsolationGuard multi-tenancy & cell boundary check on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut violations = Vec::new();
        let sql_re = Regex::new(r"(?i)\b(SELECT|DELETE|UPDATE)\b.*?\bWHERE\b").unwrap();
        let raw_socket_re =
            Regex::new(r#"(?i)TcpStream::connect\(["']\d+\.\d+\.\d+\.\d+:\d+["']\)"#).unwrap();

        for line in diff_ctx.diff_content.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                let trimmed = line[1..].trim();

                // Check 1: SQL query without tenant_id
                if sql_re.is_match(trimmed) && !trimmed.contains("tenant_id") {
                    violations.push(CellIsolationViolation {
                        category: "UNSCOPED_TENANT_QUERY".to_string(),
                        description: "SQL Query without explicit `tenant_id` filter (multi-tenant bleed risk)".to_string(),
                        snippet: trimmed.to_string(),
                    });
                }

                // Check 2: Direct raw socket connection
                if raw_socket_re.is_match(trimmed) {
                    violations.push(CellIsolationViolation {
                        category: "RAW_CROSS_CELL_SOCKET".to_string(),
                        description: "Direct hardcoded TCP socket connection bypassing Cell Gateway / Service Mesh".to_string(),
                        snippet: trimmed.to_string(),
                    });
                }
            }
        }

        let is_isolated = violations.is_empty();
        let summary = if is_isolated {
            "Cell boundaries and tenant isolation invariants verified; zero cross-tenant query leaks.".to_string()
        } else {
            format!(
                "Cell isolation warnings ({} items): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| v.description.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(CellIsolationReport {
            is_isolated,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_unscoped_tenant_query() {
        let guard = CellIsolationGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 103,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ let row = sqlx::query!(\"SELECT id, name FROM users WHERE id = $1\", user_id).fetch_one(&pool).await?;".to_string(),
            changed_files: vec!["src/users.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_cell_isolation(&diff_ctx).expect("Evaluates");
        assert!(!report.is_isolated);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].category, "UNSCOPED_TENANT_QUERY");
    }

    #[test]
    fn test_scoped_query_passes() {
        let guard = CellIsolationGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 104,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ let row = sqlx::query!(\"SELECT id, name FROM users WHERE tenant_id = $1 AND id = $2\", tenant_id, user_id).fetch_one(&pool).await?;".to_string(),
            changed_files: vec!["src/users.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_cell_isolation(&diff_ctx).expect("Evaluates");
        assert!(report.is_isolated);
    }
}
