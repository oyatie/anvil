use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::self_governance::account_pool::AccountQuotaView;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStateView {
    pub server_version: String,
    pub uptime_secs: u64,
    pub watched_repos: Vec<String>,
    pub total_prs_reviewed: usize,
    pub total_gates_evaluated: usize,
    pub merge_queue_depth: usize,
    pub quota_spent_usd: f64,
    pub quota_budget_usd: f64,
    pub active_processes_count: usize,
    pub compiler_pass_at_1_ratio: f64,
    pub quality_score_mean: f64,
    pub fleet_repos: Vec<FleetRepoView>,
    pub gate_heatmap: Vec<GateHeatmapItem>,
    pub ai_bandit_models: Vec<ModelBanditView>,
    pub dora_metrics: DoraMetricsView,
    pub recent_activities: Vec<ActivityEventView>,
    pub merge_train: Vec<MergeTrainItemView>,
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
    pub head_sha: String,
    pub open_prs: usize,
    pub pass_rate: f64,
    pub lead_time_hours: f64,
    pub deploy_frequency_per_day: f64,
    pub health_badge: String,
    pub branch_shas: HashMap<String, String>,
    pub gate_failures: Vec<GateHeatmapItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateHeatmapItem {
    pub gate_number: usize,
    pub gate_name: String,
    pub fail_count: usize,
    pub pass_percentage: f64,
    pub mutation_kill_rate: f64,
    pub category: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBanditView {
    pub model_name: String,
    pub empirical_trials: usize,
    pub empirical_pass_at_1: f64,
    pub bayesian_posterior_pass_at_1: f64,
    pub avg_cost_per_pr: f64,
    pub p99_latency_sec: f64,
    pub ucb1_score: f64,
    pub statistical_power: f64,
    pub p_value: f64,
    pub is_statistically_significant: bool,
    pub significance_badge: String,
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

pub struct HyperscalerDashboardRenderer;

/// Backward-compatibility alias
pub type LeptosDashboardRenderer = HyperscalerDashboardRenderer;

impl HyperscalerDashboardRenderer {
    /// Renders Tier-0 Hyperscaler DevOps Cockpit with Real-Time Reactive Hydration & Account Pool Controls
    pub fn render_html(state: &DashboardStateView) -> String {
        let repo_cards = crate::dashboard::panel_formatters::build_repo_cards(state);
        let merge_train_rows = crate::dashboard::panel_formatters::build_merge_train_rows(state);
        let gate_cells = crate::dashboard::panel_formatters::build_gate_cells(state);
        let account_quota_rows = crate::dashboard::panel_formatters::build_account_quota_rows(state);
        let model_rows = crate::dashboard::panel_formatters::build_model_rows(state);
        let activity_rows = crate::dashboard::panel_formatters::build_activity_rows(state);

        let css_styles = crate::dashboard::styles::get_cockpit_css();
        let client_scripts = crate::dashboard::client_scripts::get_client_scripts();

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Oyatie Anvil | Tier-0 Hyperscaler Fleet Control Plane</title>
    <style>
{}
    </style>
    <script>
{}
    </script>
</head>
<body>"#,
            css_styles,
            client_scripts
        ) + &format!(
            r#"
    <!-- TOP STATUS HERO BAR -->
    <div class="top-hero-bar">
        <div class="brand-cluster">
            <span class="brand-title">⚡ OYATIE ANVIL FLEET CONTROL PLANE</span>
            <span style="font-size: 11px; color: var(--text-muted);">v{}</span>
        </div>
        <div class="dora-kpis">
            <div class="dora-metric">
                <span class="dora-lbl">Lead Time</span>
                <span class="dora-num" id="lead-time-val">{:.1}h</span>
            </div>
            <div class="dora-metric">
                <span class="dora-lbl">Deploy Cadence</span>
                <span class="dora-num" id="deploy-cadence-val">{:.1}/d</span>
            </div>
            <div class="dora-metric">
                <span class="dora-lbl">MTTR</span>
                <span class="dora-num" id="mttr-val">{:.0}m</span>
            </div>
            <div class="dora-metric">
                <span class="dora-lbl">Failure Rate</span>
                <span class="dora-num" id="failure-rate-val">{:.1}%</span>
            </div>
        </div>
        <div class="socket-status">
            <span class="pulse-dot"></span>
            <span>Blue/Green SO_REUSEPORT (Active)</span>
        </div>
    </div>

    <!-- 4-QUADRANT HIGH-DENSITY COCKPIT -->
    <div class="cockpit-quadrant-grid">
        <!-- QUADRANT 1: MULTI-REPO TOPOLOGY & GITOPS PROMOTION DAGs -->
        <div class="panel-card">
            <div class="panel-header">
                <span class="panel-title">🌐 Panel 1: Multi-Repo Topology &amp; GitOps Promotion DAGs</span>
                <span class="badge badge-healthy">3 Repos Active</span>
            </div>
            <div class="panel-body">
                {}
            </div>
        </div>

        <!-- QUADRANT 2: SPECULATIVE MERGE QUEUE TRAIN VISUALIZER -->
        <div class="panel-card">
            <div class="panel-header">
                <span class="panel-title">🚂 Panel 2: Speculative Merge Queue Train Visualizer</span>
                <span class="badge badge-queued">Speculative Rebase Active</span>
            </div>
            <div class="panel-body">
                {}
            </div>
        </div>

        <!-- QUADRANT 3: 70-GATE CONTINUOUS GOVERNANCE & MUTATION KILL RATE MATRIX -->
        <div class="panel-card">
            <div class="panel-header">
                <span class="panel-title">🛡️ Panel 3: 70-Gate Continuous Governance &amp; Mutation Matrix</span>
                <span class="badge badge-healthy">100% Mutation Kill Rate</span>
            </div>
            <div class="gate-grid-container">
                {}
            </div>
        </div>

        <!-- QUADRANT 4: AI ROUTING BANDIT & PARETO FRONTIER -->
        <div class="panel-card">
            <div class="panel-header">
                <span class="panel-title">🤖 Panel 4: AI Model Routing Bandit (Empirical + Bayesian Bayes)</span>
                <span class="badge badge-warning">Cold Start (N=0)</span>
            </div>
            <table>
                <thead>
                    <tr>
                        <th>Model Family</th>
                        <th>Obs (N)</th>
                        <th>Pass@1</th>
                        <th>Bayes μ</th>
                        <th>Cost/PR</th>
                        <th>Latency</th>
                        <th>UCB1</th>
                        <th>Significance</th>
                    </tr>
                </thead>
                <tbody>
                    {}
                </tbody>
            </table>
        </div>
    </div>

    <!-- PANEL 5: MULTI-ACCOUNT POOL & 5-HOUR / WEEKLY QUOTA HUD -->
    <div class="panel-card" style="margin-bottom: 16px;">
        <div class="panel-header">
            <span class="panel-title">💳 Panel 5: Multi-Account Pool &amp; Time-Horizon Quota HUD (5-Hour Rolling &amp; 7-Day Weekly)</span>
            <button class="btn-add-account" onclick="openAddAccountModal()">➕ Add Account to Pool</button>
        </div>
        <table>
            <thead>
                <tr>
                    <th>Account ID</th>
                    <th>Provider</th>
                    <th>5-Hour Rolling Token Usage</th>
                    <th>Weekly Budget Spend</th>
                    <th>Status</th>
                    <th>Drain / Resume</th>
                </tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>
    </div>

    <!-- SUB-SECOND SSE AUDIT LOG STREAM -->
    <div class="panel-card">
        <div class="panel-header">
            <span class="panel-title">📡 Sub-Second Server-Sent Events (SSE) Audit Log &amp; Attestation Stream</span>
            <span class="badge badge-healthy">Live Stream</span>
        </div>
        <table>
            <thead>
                <tr>
                    <th>Timestamp</th>
                    <th>Repository</th>
                    <th>Entity</th>
                    <th>Action</th>
                    <th>Status</th>
                </tr>
            </thead>
            <tbody id="activity-tbody">
                {}
            </tbody>
        </table>
    </div>

    <!-- NATIVE MODAL DIALOG: ADD ACCOUNT TO POOL -->
    <dialog id="add-account-dialog">
        <form onsubmit="submitAddAccount(event)">
            <h3 style="margin-bottom: 14px; font-size: 15px; color: var(--accent-cyan);">➕ Register Account into Pool</h3>
            
            <div class="form-group">
                <label for="acc-id">Account ID</label>
                <input class="form-control" type="text" id="acc-id" placeholder="e.g. claude-account-3" required />
            </div>

            <div class="form-group">
                <label for="acc-provider">Model Provider</label>
                <select class="form-control" id="acc-provider">
                    <option value="claude">Anthropic Claude Code (Opus 5 / Sonnet 3.7)</option>
                    <option value="codex">OpenAI Codex (GPT-5.6sol / O3)</option>
                    <option value="antigravity">Google Antigravity (Gemini 3.7 Flash / Pro)</option>
                    <option value="cursor">Cursor Agent (Cursor Grok 4.6 / Sonnet)</option>
                    <option value="grok">xAI Grok (Grok 4.6 Fast / Reasoner)</option>
                </select>
            </div>

            <div class="form-group">
                <label for="acc-authtype">Authentication Mode</label>
                <select class="form-control" id="acc-authtype">
                    <option value="oauth">OAuth Token / Passthrough (e.g. Bearer token)</option>
                    <option value="config_dir">Config Directory (e.g. ~/.claude-seat2, CODEX_HOME)</option>
                    <option value="api_key">API Key (Direct Provider Key)</option>
                    <option value="cli">Host CLI Default</option>
                </select>
            </div>

            <div class="form-group">
                <label for="acc-oauth">OAuth Token / Key Value</label>
                <input class="form-control" type="password" id="acc-oauth" placeholder="e.g. sk-ant-oat01-..." />
            </div>

            <div class="form-group">
                <label for="acc-config-dir">Config Directory Path (Optional)</label>
                <input class="form-control" type="text" id="acc-config-dir" placeholder="e.g. /Users/name/.claude-seat2" />
            </div>

            <div class="form-group">
                <label for="acc-5hr">5-Hour Token Quota Ceiling</label>
                <input class="form-control" type="number" id="acc-5hr" value="1000000" />
            </div>

            <div class="form-group">
                <label for="acc-budget">Weekly Financial Budget ($ USD)</label>
                <input class="form-control" type="number" id="acc-budget" value="100.0" step="10.0" />
            </div>

            <div class="modal-actions">
                <button type="button" class="btn-action" style="background: rgba(255,255,255,0.1);" onclick="closeAddAccountModal()">Cancel</button>
                <button type="submit" class="btn-action" style="background: var(--accent-cyan); color: #000; font-weight: 800;">Register Account</button>
            </div>
        </form>
    </dialog>
</body>
</html>"#,
            state.server_version,
            state.dora_metrics.lead_time_hours,
            state.dora_metrics.deployment_frequency_per_day,
            state.dora_metrics.mttr_minutes,
            state.dora_metrics.change_failure_rate_pct,
            repo_cards,
            merge_train_rows,
            gate_cells,
            model_rows,
            account_quota_rows,
            activity_rows
        )
    }
}
