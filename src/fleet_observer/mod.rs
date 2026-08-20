use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::info;

use crate::github::GitHubClient;
use crate::telemetry_store::{DoraMetricSnapshot, TelemetryStore};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoFleetSummary {
    pub repo_name: String,
    pub active_branch_head_sha: String,
    pub open_pr_count: usize,
    pub merge_queue_depth: usize,
    pub pass_rate_percent: f64,
    pub dora_metrics: DoraMetricSnapshot,
    pub gate_failure_top3: Vec<(String, usize)>,
    pub branch_shas: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetOverviewReport {
    pub total_managed_repos: usize,
    pub repos: Vec<RepoFleetSummary>,
    pub global_dora: DoraMetricSnapshot,
}

#[derive(Clone)]
struct CachedFleetData {
    timestamp: Instant,
    report: FleetOverviewReport,
}

pub struct FleetObserver {
    pub github_client: Arc<GitHubClient>,
    pub telemetry_store: Arc<TelemetryStore>,
    cache: Arc<RwLock<Option<CachedFleetData>>>,
}

impl FleetObserver {
    pub fn new(github_client: Arc<GitHubClient>, telemetry_store: Arc<TelemetryStore>) -> Self {
        Self {
            github_client,
            telemetry_store,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Aggregates fleet-wide live telemetry across all managed repositories with a 15-second TTL cache
    pub async fn aggregate_fleet_overview(
        &self,
        managed_repos: &[String],
    ) -> Result<FleetOverviewReport> {
        // Check cache with 15s TTL
        {
            let guard = self.cache.read().await;
            if let Some(cached) = guard.as_ref() {
                if cached.timestamp.elapsed() < Duration::from_secs(15) {
                    return Ok(cached.report.clone());
                }
            }
        }

        let mut summaries = Vec::new();

        for repo in managed_repos {
            // Fetch live DORA metrics directly from GitHub facts for this repo
            let dora = self
                .github_client
                .fetch_repo_dora_metrics(repo)
                .await
                .unwrap_or_else(|_| crate::telemetry_store::DoraMetricSnapshot {
                    repo: repo.clone(),
                    timestamp: chrono::Utc::now(),
                    lead_time_for_changes_hours: 0.0,
                    deployment_frequency_per_day: 0.0,
                    change_failure_rate_percent: 0.0,
                    mean_time_to_restore_mins: 0.0,
                    total_deployments_30d: 0,
                    total_incidents_30d: 0,
                });

            let heatmap = self.telemetry_store.get_gate_failure_heatmap(repo).await;

            let mut sorted_failures: Vec<_> = heatmap.into_iter().collect();
            sorted_failures.sort_by_key(|b| std::cmp::Reverse(b.1));
            let top3: Vec<_> = sorted_failures.into_iter().take(3).collect();

            // Fetch live open PRs from GitHub
            let open_prs = self
                .github_client
                .list_open_prs(repo)
                .await
                .unwrap_or_default();
            let open_pr_count = open_prs.len();

            // Fetch live branch commit SHAs
            let mut branch_shas = HashMap::new();
            for branch in &["dev", "staging", "canary", "production", "main"] {
                if let Ok(sha) = self.github_client.fetch_branch_sha(repo, branch).await {
                    let short_sha = if sha.len() >= 7 {
                        sha[..7].to_string()
                    } else {
                        sha
                    };
                    branch_shas.insert(branch.to_string(), short_sha);
                }
            }

            let head_sha = branch_shas
                .get("dev")
                .or_else(|| branch_shas.get("main"))
                .cloned()
                .unwrap_or_else(|| "HEAD".to_string());

            let merge_queue_depth = self
                .github_client
                .fetch_merge_queue_depth(repo, "main")
                .await
                .unwrap_or(0);

            // Compute empirical pass rate from actual change failure rate
            let pass_rate_percent = (100.0 - dora.change_failure_rate_percent).clamp(0.0, 100.0);

            summaries.push(RepoFleetSummary {
                repo_name: repo.clone(),
                active_branch_head_sha: head_sha,
                open_pr_count,
                merge_queue_depth,
                pass_rate_percent,
                dora_metrics: dora,
                gate_failure_top3: top3,
                branch_shas,
            });
        }

        // Aggregate Global Fleet DORA metrics across all repositories
        let total_repos_count = summaries.len().max(1);
        let global_lead_time: f64 = summaries
            .iter()
            .map(|s| s.dora_metrics.lead_time_for_changes_hours)
            .sum::<f64>()
            / total_repos_count as f64;
        let global_deploy_cadence: f64 = summaries
            .iter()
            .map(|s| s.dora_metrics.deployment_frequency_per_day)
            .sum::<f64>();
        let global_cfr: f64 = summaries
            .iter()
            .map(|s| s.dora_metrics.change_failure_rate_percent)
            .sum::<f64>()
            / total_repos_count as f64;
        let global_mttr: f64 = summaries
            .iter()
            .map(|s| s.dora_metrics.mean_time_to_restore_mins)
            .sum::<f64>()
            / total_repos_count as f64;

        let global_dora = crate::telemetry_store::DoraMetricSnapshot {
            repo: "fleet_global".to_string(),
            timestamp: chrono::Utc::now(),
            lead_time_for_changes_hours: global_lead_time,
            deployment_frequency_per_day: global_deploy_cadence,
            change_failure_rate_percent: global_cfr,
            mean_time_to_restore_mins: global_mttr,
            total_deployments_30d: summaries
                .iter()
                .map(|s| s.dora_metrics.total_deployments_30d)
                .sum(),
            total_incidents_30d: summaries
                .iter()
                .map(|s| s.dora_metrics.total_incidents_30d)
                .sum(),
        };

        info!(
            "🌐 [Fleet Observer] Dynamically aggregated live telemetry across {} repositories.",
            managed_repos.len()
        );

        let report = FleetOverviewReport {
            total_managed_repos: managed_repos.len(),
            repos: summaries,
            global_dora,
        };

        // Update cache
        {
            let mut guard = self.cache.write().await;
            *guard = Some(CachedFleetData {
                timestamp: Instant::now(),
                report: report.clone(),
            });
        }

        Ok(report)
    }
}
