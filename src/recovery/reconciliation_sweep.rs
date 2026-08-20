use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{info, warn};

use crate::github::GitHubClient;
use crate::state::StateManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenPrSummary {
    pub number: u64,
    pub title: String,
    pub head_sha: String,
    pub head_ref_name: String,
    pub is_draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutageRecoveryReport {
    pub repos_scanned: Vec<String>,
    pub total_prs_inspected: usize,
    pub prs_requiring_certification: Vec<u64>,
    pub prs_requiring_review: Vec<u64>,
    pub uncertified_prs_details: Vec<(String, OpenPrSummary)>,
    pub issues_reconciled: usize,
    pub duration_secs: f64,
    pub status: String,
}

#[derive(Clone)]
pub struct OutageRecoveryReconciler {
    github_client: Arc<GitHubClient>,
    state_mgr: Arc<StateManager>,
}

impl OutageRecoveryReconciler {
    pub fn new(github_client: Arc<GitHubClient>, state_mgr: Arc<StateManager>) -> Self {
        Self {
            github_client,
            state_mgr,
        }
    }

    /// Executes a comprehensive repository-wide reconciliation sweep on boot or after partial outages
    pub async fn run_full_sweep(&self, watched_repos: &[String]) -> Result<OutageRecoveryReport> {
        let start = Instant::now();
        info!("==========================================================");
        info!(
            "🔄 [OUTAGE RECOVERY] Initiating Full Reconciliation Sweep across {:?}...",
            watched_repos
        );
        info!("==========================================================");

        let mut total_prs = 0;
        let mut prs_cert = Vec::new();
        let prs_rev = Vec::new();
        let mut uncertified_prs_details = Vec::new();
        let mut total_issues_reconciled = 0;

        for repo in watched_repos {
            info!(
                "🔍 [Outage Recovery] Scanning open PRs for repository '{}'...",
                repo
            );
            match self.fetch_open_prs(repo).await {
                Ok(open_prs) => {
                    total_prs += open_prs.len();
                    for pr in open_prs {
                        if pr.is_draft {
                            continue;
                        }

                        // Inspect persistent state to detect uncertified or stale PRs
                        let state_opt = self.state_mgr.get_pr_state(repo, pr.number).await;
                        let needs_cert = match state_opt {
                            Some(st) => st.last_certified_head_sha.as_deref() != Some(&pr.head_sha),
                            None => true,
                        };

                        if needs_cert {
                            info!(
                                "⚡ [Outage Recovery] Detected uncertified PR {}#{} (head_sha: {}). Queuing for certification...",
                                repo, pr.number, pr.head_sha
                            );
                            prs_cert.push(pr.number);
                            uncertified_prs_details.push((repo.clone(), pr.clone()));
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "⚠️ [Outage Recovery] Failed to fetch open PRs for {}: {}",
                        repo, e
                    );
                }
            }

            // Sweep and reconcile open issues against masterplan.json
            info!(
                "🔍 [Outage Recovery] Reconciling open issues for repository '{}'...",
                repo
            );
            let reconciler =
                crate::issue_reconciler::IssueReconciler::new(self.github_client.clone());
            match reconciler.reconcile_issues(repo).await {
                Ok(reconciled) => {
                    total_issues_reconciled += reconciled.len();
                    info!(
                        "✅ [Outage Recovery] Reconciled {} issues for {}",
                        reconciled.len(),
                        repo
                    );
                }
                Err(e) => {
                    warn!(
                        "⚠️ [Outage Recovery] Issue reconciliation for {} failed: {}",
                        repo, e
                    );
                }
            }
        }

        let elapsed = start.elapsed();
        let report = OutageRecoveryReport {
            repos_scanned: watched_repos.to_vec(),
            total_prs_inspected: total_prs,
            prs_requiring_certification: prs_cert,
            prs_requiring_review: prs_rev,
            uncertified_prs_details,
            issues_reconciled: total_issues_reconciled,
            duration_secs: elapsed.as_secs_f64(),
            status: "COMPLETED".to_string(),
        };

        info!(
            "🎉 [Outage Recovery] Sweep complete in {:.2}s. Inspected {} PRs ({} uncertified), reconciled {} issues.",
            report.duration_secs, report.total_prs_inspected, report.prs_requiring_certification.len(), report.issues_reconciled
        );

        Ok(report)
    }

    /// Fetches all open PRs using GitHub CLI
    async fn fetch_open_prs(&self, repo: &str) -> Result<Vec<OpenPrSummary>> {
        let mut list_cmd = tokio::process::Command::new("gh");
        list_cmd.args([
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--json",
            "number,title,headRefOid,headRefName,isDraft",
        ]);
        let output = crate::exec::run_bounded(
            list_cmd,
            crate::exec::ExecClass::Api,
            "gh pr list (reconciliation sweep)",
        )
        .await
        .context("Failed to execute gh pr list")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gh pr list failed: {}", stderr);
        }

        #[derive(Deserialize)]
        struct GhPrItem {
            number: u64,
            title: String,
            #[serde(rename = "headRefOid")]
            head_ref_oid: String,
            #[serde(rename = "headRefName")]
            head_ref_name: String,
            #[serde(rename = "isDraft")]
            is_draft: bool,
        }

        let items: Vec<GhPrItem> = serde_json::from_slice(&output.stdout).unwrap_or_default();
        Ok(items
            .into_iter()
            .map(|i| OpenPrSummary {
                number: i.number,
                title: i.title,
                head_sha: i.head_ref_oid,
                head_ref_name: i.head_ref_name,
                is_draft: i.is_draft,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_recovery_report_structure() {
        let report = OutageRecoveryReport {
            repos_scanned: vec!["oyatie/anvil".to_string()],
            total_prs_inspected: 5,
            prs_requiring_certification: vec![5, 6],
            prs_requiring_review: vec![5],
            uncertified_prs_details: Vec::new(),
            issues_reconciled: 12,
            duration_secs: 1.45,
            status: "COMPLETED".to_string(),
        };

        assert_eq!(report.total_prs_inspected, 5);
        assert_eq!(report.prs_requiring_certification.len(), 2);
    }
}
