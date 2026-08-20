use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

pub mod issue_auditor;
pub use issue_auditor::{IssueAuditFinding, IssueAuditStatus, IssueAuditor};

use crate::github::GitHubClient;

#[derive(Debug, Clone)]
pub struct ReconciledIssue {
    pub issue_number: u64,
    pub title: String,
    pub status: String,
    pub resolution_reason: String,
}

pub struct IssueReconciler {
    #[allow(dead_code)]
    github_client: Arc<GitHubClient>,
}

impl IssueReconciler {
    pub fn new(github_client: Arc<GitHubClient>) -> Self {
        Self { github_client }
    }

    /// Scans open issues on the repository, checks trunk CI status and apex ADRs, and auto-closes stale reports
    pub async fn reconcile_issues(&self, repo: &str) -> Result<Vec<ReconciledIssue>> {
        info!(
            "Scanning open issues on {} for auto-reconciliation...",
            repo
        );

        let mut list_cmd = Command::new("gh");
        list_cmd.args([
            "issue",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--json",
            "number,title,body,createdAt",
            "--limit",
            "100",
        ]);
        let output =
            crate::exec::run_bounded(list_cmd, crate::exec::ExecClass::Api, "gh issue list")
                .await
                .context("Failed to list open issues via gh")?;

        if !output.status.success() {
            warn!(
                "Failed to fetch issues from {}: {:?}",
                repo,
                String::from_utf8_lossy(&output.stderr)
            );
            return Ok(Vec::new());
        }

        let issues: Vec<serde_json::Value> =
            serde_json::from_slice(&output.stdout).unwrap_or_default();

        let mut reconciled = Vec::new();

        for issue in issues {
            let number = issue["number"].as_u64().unwrap_or(0);
            let title = issue["title"].as_str().unwrap_or("");
            let body = issue["body"].as_str().unwrap_or("");

            let finding = IssueAuditor::audit_issue(Path::new("."), number, title, body);

            if finding.status != IssueAuditStatus::Active {
                info!(
                    "Auto-reconciling issue #{} on {}: {}",
                    number, repo, finding.resolution_reason
                );
                let mut close_cmd = Command::new("gh");
                close_cmd.args([
                    "issue",
                    "close",
                    &number.to_string(),
                    "--repo",
                    repo,
                    "--comment",
                    &format!(
                        "🤖 **Autonomous Anvil Issue Reconciliation**\n\n**Status:** Auto-closed\n**Reason:** {}\n**Verification Receipt:** `{}`\n\n---\n*🤖 [Reconciled] by Oyatie Anvil*",
                        finding.resolution_reason,
                        finding.resolution_receipt.as_deref().unwrap_or("N/A")
                    ),
                ]);
                let _ = crate::exec::run_bounded(
                    close_cmd,
                    crate::exec::ExecClass::Api,
                    "gh issue close (reconciler)",
                )
                .await;

                reconciled.push(ReconciledIssue {
                    issue_number: number,
                    title: title.to_string(),
                    status: format!("{:?}", finding.status),
                    resolution_reason: finding.resolution_reason,
                });
            }
        }

        Ok(reconciled)
    }
}
