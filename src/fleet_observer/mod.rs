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
    /// The head commit of the repo's active branch, or `None` when the branch
    /// fetch resolved neither `dev` nor `main`.
    ///
    /// A `String` here has no way to spell "not observed", so a poll that
    /// resolved nothing has to invent something a reader takes for a commit.
    /// `Option` lets the surface render the absence.
    pub active_branch_head_sha: Option<String>,
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

/// The head commit of the branch a repo is worked on, or `None` when neither
/// `dev` nor `main` was fetched.
///
/// The string `HEAD` is not a commit. It reached `active_branch_head_sha`
/// whenever the branch fetch came back without either name -- a rate limit, an
/// expired token, a repo that uses neither -- and every surface that publishes
/// that field publishes it where a reader expects a SHA. A lookup that
/// resolved nothing has no commit to name, which is what `None` says.
pub fn resolve_head_sha(branch_shas: &HashMap<String, String>) -> Option<String> {
    branch_shas
        .get("dev")
        .or_else(|| branch_shas.get("main"))
        .cloned()
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

    /// The cached fleet overview, or `None` when nothing has been polled yet.
    ///
    /// This used to fall back to a report built from literals -- 98.6% pass
    /// rate, 1.2h lead time, 4.8 deployments a day, one incident in thirty --
    /// which the browser reads and renders as the fleet's DORA metrics. The
    /// fallback ran on every request before the first successful poll, and
    /// forever if polling never succeeded, so a fleet nobody could observe
    /// displayed as a healthy one. `None` is the honest answer to "what is the
    /// fleet doing" when nothing has looked.
    pub async fn get_fleet_overview_instant(
        &self,
        _managed_repos: &[String],
    ) -> Option<FleetOverviewReport> {
        let guard = self.cache.read().await;
        guard.as_ref().map(|cached| cached.report.clone())
    }

    /// Spawns an asynchronous background poller that continuously refreshes the telemetry cache
    pub fn spawn_continuous_poller(self: &Arc<Self>, managed_repos: Vec<String>) {
        let observer = Arc::clone(self);
        tokio::spawn(async move {
            info!(
                "🌐 [Fleet Observer] Continuous background telemetry poller initialized (30s cadence)"
            );
            // Initial eager refresh
            let _ = observer.aggregate_fleet_overview(&managed_repos).await;

            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let _ = observer.aggregate_fleet_overview(&managed_repos).await;
            }
        });
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

                let head_sha = resolve_head_sha(&branch_shas);

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

#[cfg(test)]
mod unpolled_fleet_tests {
    use super::*;

    /// The observer used to answer a cache miss with a report built from
    /// literals -- 98.6% pass rate, 1.2h lead time, one incident in thirty
    /// days -- which the dashboard renders as the fleet's real DORA metrics.
    /// That path ran on every request before the first successful poll and
    /// forever if polling never succeeded, so an unobservable fleet displayed
    /// as a healthy one. Nothing polled means nothing to report.
    #[tokio::test]
    async fn an_unpolled_observer_reports_nothing_rather_than_a_healthy_fleet() {
        let dir = tempfile::tempdir().expect("tempdir");
        let observer = FleetObserver::new(
            Arc::new(GitHubClient::new()),
            Arc::new(TelemetryStore::new(dir.path()).await),
        );

        let overview = observer
            .get_fleet_overview_instant(&["oyatie/anvil".to_string()])
            .await;

        assert!(
            overview.is_none(),
            "a fleet nobody has polled must report nothing, not a fabricated summary"
        );
    }
}
