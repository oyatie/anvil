//! The view model the dashboard publishes: the structs the SSR page renders
//! and `/api/state` serializes verbatim.
//!
//! Split from `ssr_renderer` to hold the renderer inside D-35's budget. A
//! field here is a published claim, so a value that was never observed has to
//! be representable as absent rather than substituted for.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::self_governance::account_pool::AccountQuotaView;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DashboardStateView {
    pub server_version: String,
    pub watched_repos: Vec<String>,
    pub total_gates_evaluated: usize,
    pub merge_queue_depth: usize,
    pub quota_spent_usd: f64,
    pub quota_budget_usd: f64,
    pub fleet_repos: Vec<FleetRepoView>,
    pub gate_heatmap: Vec<GateHeatmapItem>,
    /// `None` until the fleet poller lands a successful sweep. The client
    /// already guards on this being present; the SSR path renders an em dash.
    pub dora_metrics: Option<DoraMetricsView>,
    pub recent_activities: Vec<ActivityEventView>,
    pub merge_train: Vec<MergeTrainItemView>,
    /// Repos whose open-pull-request fetch failed, so `merge_train` carries no
    /// row for them. An empty queue and a query that never answered are not
    /// the same state, and the panel has to be able to tell them apart.
    pub unobserved_merge_train_repos: Vec<String>,
    pub account_quotas: Vec<AccountQuotaView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeTrainItemView {
    pub repo: String,
    pub pr_number: u64,
    pub title: String,
    pub speculative_base: String,
    pub head_sha: String,
    pub state: String,
    pub gates_completed: usize,
    pub total_gates: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetRepoView {
    pub name: String,
    /// `None` when the poller resolved no head commit for this repo. The field
    /// is published on the JSON state surface, where a reader takes it for a
    /// commit, so an unresolved lookup has to serialize as absent.
    pub head_sha: Option<String>,
    pub open_prs: usize,
    pub pass_rate: f64,
    pub lead_time_hours: f64,
    pub deploy_frequency_per_day: f64,
    pub branch_shas: HashMap<String, String>,
    pub gate_failures: Vec<GateHeatmapItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateHeatmapItem {
    pub gate_number: usize,
    pub gate_name: String,
    pub fail_count: usize,
    pub pass_percentage: f64,
    pub category: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoraMetricsView {
    pub deployment_frequency_per_day: f64,
    pub lead_time_hours: f64,
    pub change_failure_rate_pct: f64,
    pub mttr_minutes: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityEventView {
    pub timestamp: String,
    pub repo: String,
    pub entity: String,
    pub action: String,
    pub status: String,
}
