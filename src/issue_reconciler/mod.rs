use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
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

        let mut list_cmd = crate::exec::gh();
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
                    "Proposing resolution for issue #{} on {}: {}",
                    number, repo, finding.resolution_reason
                );
                // Propose, do not close.
                //
                // `IssueAuditor` reaches its verdict from a title/body substring match
                // alone -- `ResolvedByCommit` in particular publishes "Trunk CI is green
                // and passing all gates" without ever querying CI. Closing another
                // team's issue on an unevaluated factual claim is not recoverable by the
                // reader, who sees a confident reason and no way to know it was never
                // checked. Until each verdict is backed by a real signal, Anvil states
                // its finding and a human decides.
                let mut close_cmd = crate::exec::gh();
                close_cmd.args([
                    "issue",
                    "comment",
                    &number.to_string(),
                    "--repo",
                    repo,
                    "--body",
                    &format!(
                        "**Proposed resolution:** close as `{:?}`\n\n\
                         **Basis:** {}\n\n\
                         **How this was determined:** issue title/body pattern match. \
                         Anvil did not independently verify the underlying condition, so \
                         this is a proposal, not a finding. Close if it reflects reality.\n\n\
                         **Receipt:** `{}`\n\n---\n*[Reconciled] by Oyatie Anvil*",
                        finding.status,
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
