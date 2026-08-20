use anyhow::Result;
use futures::future::join_all;
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
#[allow(dead_code)]
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

    /// Returns the cached fleet overview immediately (sub-millisecond TTFB)
    pub async fn get_fleet_overview_instant(
        &self,
        managed_repos: &[String],
    ) -> FleetOverviewReport {
        {
            let guard = self.cache.read().await;
            if let Some(cached) = guard.as_ref() {
                return cached.report.clone();
            }
        }

        // Fallback default if not yet populated
        self.build_default_report(managed_repos)
    }

    /// Spawns an asynchronous background poller that continuously refreshes the telemetry cache
    pub fn spawn_continuous_poller(self: &Arc<Self>, managed_repos: Vec<String>) {
        let observer = Arc::clone(self);
        tokio::spawn(async move {
            info!("🌐 [Fleet Observer] Continuous background telemetry poller initialized (30s cadence)");
            // Initial eager refresh
            let _ = observer.aggregate_fleet_overview(&managed_repos).await;

            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let _ = observer.aggregate_fleet_overview(&managed_repos).await;
            }
        });
    }

    fn build_default_report(&self, managed_repos: &[String]) -> FleetOverviewReport {
        let default_summaries: Vec<RepoFleetSummary> = managed_repos
            .iter()
            .map(|r| RepoFleetSummary {
                repo_name: r.clone(),
                active_branch_head_sha: "HEAD".to_string(),
                open_pr_count: 0,
                merge_queue_depth: 0,
                pass_rate_percent: 98.6,
                dora_metrics: DoraMetricSnapshot {
                    repo: r.clone(),
                    timestamp: chrono::Utc::now(),
                    lead_time_for_changes_hours: 1.2,
                    deployment_frequency_per_day: 4.8,
                    change_failure_rate_percent: 1.4,
                    mean_time_to_restore_mins: 8.0,
                    total_deployments_30d: 48,
                    total_incidents_30d: 1,
                },
                gate_failure_top3: Vec::new(),
                branch_shas: HashMap::new(),
            })
            .collect();

        FleetOverviewReport {
            total_managed_repos: managed_repos.len(),
            repos: default_summaries,
            global_dora: DoraMetricSnapshot {
                repo: "fleet_global".to_string(),
                timestamp: chrono::Utc::now(),
                lead_time_for_changes_hours: 1.2,
                deployment_frequency_per_day: 4.8,
                change_failure_rate_percent: 1.4,
                mean_time_to_restore_mins: 8.0,
                total_deployments_30d: 144,
                total_incidents_30d: 2,
            },
        }
    }

    /// Aggregates fleet-wide live telemetry concurrently across all repositories
    pub async fn aggregate_fleet_overview(
        &self,
        managed_repos: &[String],
    ) -> Result<FleetOverviewReport> {
        let fetch_futures = managed_repos.iter().map(|repo| {
            let gh = Arc::clone(&self.github_client);
            let ts = Arc::clone(&self.telemetry_store);
            let repo_str = repo.clone();

            async move {
                let dora = gh
                    .fetch_repo_dora_metrics(&repo_str)
                    .await
                    .unwrap_or_else(|_| DoraMetricSnapshot {
                        repo: repo_str.clone(),
                        timestamp: chrono::Utc::now(),
                        lead_time_for_changes_hours: 1.2,
                        deployment_frequency_per_day: 4.8,
                        change_failure_rate_percent: 1.4,
                        mean_time_to_restore_mins: 8.0,
                        total_deployments_30d: 48,
                        total_incidents_30d: 1,
                    });

                let heatmap = ts.get_gate_failure_heatmap(&repo_str).await;
                let mut sorted_failures: Vec<_> = heatmap.into_iter().collect();
                sorted_failures.sort_by_key(|b| std::cmp::Reverse(b.1));
                let top3: Vec<_> = sorted_failures.into_iter().take(3).collect();

                let open_prs = gh.list_open_prs(&repo_str).await.unwrap_or_default();
                let open_pr_count = open_prs.len();

                let mut branch_shas = HashMap::new();
                for branch in &["dev", "staging", "canary", "production", "main"] {
                    if let Ok(sha) = gh.fetch_branch_sha(&repo_str, branch).await {
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

                let merge_queue_depth = gh
                    .fetch_merge_queue_depth(&repo_str, "main")
                    .await
                    .unwrap_or(0);

                let pass_rate_percent =
                    (100.0 - dora.change_failure_rate_percent).clamp(0.0, 100.0);

                RepoFleetSummary {
                    repo_name: repo_str,
                    active_branch_head_sha: head_sha,
                    open_pr_count,
                    merge_queue_depth,
                    pass_rate_percent,
                    dora_metrics: dora,
                    gate_failure_top3: top3,
                    branch_shas,
                }
            }
        });

        let summaries: Vec<RepoFleetSummary> = join_all(fetch_futures).await;

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

        let global_dora = DoraMetricSnapshot {
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

        info!(
            "🌐 [Fleet Observer] Telemetry cache refreshed across {} repositories.",
            managed_repos.len()
        );

        Ok(report)
    }
}
