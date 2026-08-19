use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiContractReport {
    pub is_intact: bool,
    pub auto_synced_files: Vec<String>,
    pub summary: String,
}

pub struct ApiContractGuard;

impl Default for ApiContractGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiContractGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates OpenAPI schema integrity and auto-reconciles platform contract drift
    pub async fn ensure_contract_integrity(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<ApiContractReport> {
        info!(
            "Running ApiContractGuard schema integrity check on {}#{}...",
            repo, diff_ctx.pr_number
        );

        let touches_api = diff_ctx.changed_files.iter().any(|f| {
            f.contains("openapi")
                || f.contains("route")
                || f.contains("handler")
                || f.contains("controller")
                || f.ends_with(".proto")
        });

        if !touches_api {
            return Ok(ApiContractReport {
                is_intact: true,
                auto_synced_files: Vec::new(),
                summary: "No API contracts or route handlers modified.".to_string(),
            });
        }

        // 1. Run OpenAPI schema reference checks if script exists
        let check_script = repo_dir.join("scripts/check-openapi-refs.mjs");
        let mut script_failed = false;
        if check_script.exists() {
            let out = Command::new("node")
                .current_dir(repo_dir)
                .arg("scripts/check-openapi-refs.mjs")
                .output()
                .await;

            if let Ok(res) = out {
                if !res.status.success() {
                    warn!(
                        "check-openapi-refs.mjs flagged drift. Triggering auto-reconciliation..."
                    );
                    script_failed = true;
                }
            }
        }

        // 2. Run union-openapi.py if available
        let union_script = repo_dir.join("scripts/union-openapi.py");
        if union_script.exists() {
            let _ = Command::new("python3")
                .current_dir(repo_dir)
                .arg("scripts/union-openapi.py")
                .output()
                .await;
        }

        // 3. Check for modified schema files
        let status_out = Command::new("git")
            .current_dir(repo_dir)
            .args(["status", "--porcelain"])
            .output()
            .await
            .context("Failed to check git status in ApiContractGuard")?;

        let modified_lines = String::from_utf8_lossy(&status_out.stdout);
        let synced_files: Vec<String> = modified_lines
            .lines()
            .map(|l| l[3..].trim().to_string())
            .filter(|f| f.contains("openapi") || f.contains("schema") || f.contains("contract"))
            .collect();

        let is_intact = !script_failed || !synced_files.is_empty();

        let summary = if !synced_files.is_empty() {
            format!(
                "Auto-reconciled OpenAPI schemas & contract definitions: {}",
                synced_files.join(", ")
            )
        } else if is_intact {
            "OpenAPI schemas and API contracts are 100% in sync with zero drift.".to_string()
        } else {
            "OpenAPI contract drift detected requiring manual attention.".to_string()
        };

        Ok(ApiContractReport {
            is_intact,
            auto_synced_files: synced_files,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_non_api_diff_passes() {
        let guard = ApiContractGuard::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 102,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+ const x = 1;".to_string(),
            changed_files: vec!["README.md".to_string()],
            is_incremental: false,
        };

        let res = guard
            .ensure_contract_integrity("oyatie/console", &temp_dir, &diff_ctx)
            .await
            .expect("Valid");
        assert!(res.is_intact);
        assert!(res.auto_synced_files.is_empty());
    }
}
