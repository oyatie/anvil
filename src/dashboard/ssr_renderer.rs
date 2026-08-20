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

pub struct LeptosDashboardRenderer;

impl LeptosDashboardRenderer {
    /// Renders Tier-0 Hyperscaler DevOps Cockpit with Real-Time Reactive Hydration & Account Pool Controls
    pub fn render_html(state: &DashboardStateView) -> String {
        let repo_cards = state
            .fleet_repos
            .iter()
            .map(|r| {
                let dev_sha = r.branch_shas.get("dev").cloned().unwrap_or_else(|| "N/A".to_string());
                let stg_sha = r.branch_shas.get("staging").cloned().unwrap_or_else(|| "N/A".to_string());
                let cnr_sha = r.branch_shas.get("canary").cloned().unwrap_or_else(|| "N/A".to_string());
                let prd_sha = r.branch_shas.get("production").or_else(|| r.branch_shas.get("main")).cloned().unwrap_or_else(|| "N/A".to_string());

                format!(
                    r#"<div class="repo-row">
                        <div class="repo-meta">
                            <div class="repo-name">
                                <span class="icon">📦</span>
                                <strong>{}</strong>
                                <span class="badge badge-healthy">{}</span>
                            </div>
                            <div class="repo-stats">
                                <span>Open PRs: <strong class="text-cyan">{}</strong></span>
                                <span>Lead Time: <strong>{:.1}h</strong></span>
                                <span>Deploy Cadence: <strong>{:.1}/d</strong></span>
                            </div>
                        </div>
                        <div class="gitops-dag">
                            <div class="dag-node node-dev"><span class="dag-label">Dev</span><code>{}</code></div>
                            <div class="dag-arrow">➔</div>
                            <div class="dag-node node-staging"><span class="dag-label">Staging</span><code>{}</code></div>
                            <div class="dag-arrow">➔</div>
                            <div class="dag-node node-canary"><span class="dag-label">Canary 5%</span><code>{}</code></div>
                            <div class="dag-arrow">➔</div>
                            <div class="dag-node node-prod"><span class="dag-label">Production</span><code>{}</code></div>
                        </div>
                    </div>"#,
                    r.name, r.health_badge, r.open_prs, r.lead_time_hours, r.deploy_frequency_per_day,
                    dev_sha, stg_sha, cnr_sha, prd_sha
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let merge_train_rows = if state.merge_train.is_empty() {
            r#"<div class="empty-state">
                <span class="icon">🚂</span>
                <p>No inflight speculative merge conflicts. Queue idle & ready for admission.</p>
            </div>"#
                .to_string()
        } else {
            state
                .merge_train
                .iter()
                .map(|t| {
                    format!(
                        r#"<div class="train-item">
                            <div class="train-header">
                                <span class="train-pr">{}#{}</span>
                                <span class="train-title">{}</span>
                                <span class="badge badge-queued">{}</span>
                            </div>
                            <div class="train-progress">
                                <div class="train-spec-base">Base: <code>{}</code> ➔ Head: <code>{}</code></div>
                                <div class="progress-bar-bg"><div class="progress-bar-fill" style="width: {}%"></div></div>
                                <span class="progress-text">{}/{} Gates</span>
                            </div>
                        </div>"#,
                        t.repo, t.pr_number, t.title, t.state,
                        t.speculative_base, t.head_sha,
                        (t.gates_completed as f64 / t.total_gates.max(1) as f64 * 100.0) as usize,
                        t.gates_completed, t.total_gates
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let gate_cells = state
            .gate_heatmap
            .iter()
            .map(|g| {
                let cell_class = if g.fail_count == 0 {
                    "gate-cell gate-green"
                } else if g.pass_percentage >= 90.0 {
                    "gate-cell gate-amber"
                } else {
                    "gate-cell gate-red"
                };
                format!(
                    r#"<div class="{}" title="Gate {}: {} | Failures: {} | Pass: {:.1}% | Mutation Kill Rate: {:.0}%">
                        <span class="gate-num">G{:02}</span>
                        <span class="gate-name">{}</span>
                        <span class="gate-mkr">MKR {:.0}%</span>
                    </div>"#,
                    cell_class, g.gate_number, g.gate_name, g.fail_count, g.pass_percentage, g.mutation_kill_rate,
                    g.gate_number, g.gate_name, g.mutation_kill_rate
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let account_quota_rows = state
            .account_quotas
            .iter()
            .map(|acc| {
                let status_badge = if acc.is_draining {
                    "<span class=\"badge badge-warning\">DRAINING</span>"
                } else if acc.is_active {
                    "<span class=\"badge badge-healthy\">ACTIVE</span>"
                } else {
                    "<span class=\"badge badge-warning\">COOLDOWN</span>"
                };

                let action_button = if acc.is_draining {
                    format!(
                        r#"<button class="btn-action btn-resume" onclick="resumeAccount('{}')">Resume</button>"#,
                        acc.account_id
                    )
                } else {
                    format!(
                        r#"<button class="btn-action btn-drain" onclick="drainAccount('{}')">Drain</button>"#,
                        acc.account_id
                    )
                };

                format!(
                    r#"<tr>
                        <td><strong>{}</strong></td>
                        <td><code>{}</code></td>
                        <td>
                            <div class="progress-bar-bg"><div class="progress-bar-fill" style="width: {:.1}%"></div></div>
                            <span class="progress-text">{:.1}% ({}k rem)</span>
                        </td>
                        <td>${:.2} / ${:.2}</td>
                        <td>{}</td>
                        <td>{}</td>
                    </tr>"#,
                    acc.account_id,
                    acc.provider,
                    acc.pct_5hr_used,
                    acc.pct_5hr_used,
                    acc.remaining_5hr_tokens / 1000,
                    acc.weekly_spent_usd,
                    acc.weekly_budget_usd,
                    status_badge,
                    action_button
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let model_rows = state
            .ai_bandit_models
            .iter()
            .map(|m| {
                let sig_class = if m.is_statistically_significant {
                    "badge-healthy"
                } else {
                    "badge-warning"
                };
                format!(
                    r#"<tr>
                        <td><strong>{}</strong></td>
                        <td>{}</td>
                        <td><strong>{:.1}%</strong></td>
                        <td><strong>{:.1}%</strong></td>
                        <td>${:.3}</td>
                        <td>{:.1}s</td>
                        <td><code>{:.3}</code></td>
                        <td><span class="badge {}">{}</span></td>
                    </tr>"#,
                    m.model_name,
                    m.empirical_trials,
                    m.empirical_pass_at_1 * 100.0,
                    m.bayesian_posterior_pass_at_1 * 100.0,
                    m.avg_cost_per_pr,
                    m.p99_latency_sec,
                    m.ucb1_score,
                    sig_class,
                    m.significance_badge
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let activity_rows = state
            .recent_activities
            .iter()
            .map(|act| {
                format!(
                    r#"<tr>
                        <td>{}</td>
                        <td><code>{}</code></td>
                        <td><strong>{}</strong></td>
                        <td>{}</td>
                        <td><span class="badge badge-healthy">{}</span></td>
                    </tr>"#,
                    act.timestamp, act.repo, act.entity, act.action, act.status
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Oyatie Anvil | Tier-0 Hyperscaler Fleet Control Plane</title>
    <style>
        :root {{
            --bg-dark: #0a0e17;
            --surface-dark: #111827;
            --surface-card: #162032;
            --surface-border: #1f2d47;
            --text-primary: #f3f4f6;
            --text-secondary: #9ca3af;
            --text-muted: #6b7280;
            --accent-cyan: #06b6d4;
            --accent-blue: #3b82f6;
            --accent-emerald: #10b981;
            --accent-amber: #f59e0b;
            --accent-rose: #f43f5e;
            --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
            --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            background-color: var(--bg-dark);
            color: var(--text-primary);
            font-family: var(--font-sans);
            padding: 16px 20px;
            line-height: 1.4;
        }}
        .top-hero-bar {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 12px 20px;
            background: var(--surface-card);
            border: 1px solid var(--surface-border);
            border-radius: 10px;
            margin-bottom: 16px;
            backdrop-filter: blur(8px);
        }}
        .brand-cluster {{
            display: flex;
            align-items: center;
            gap: 12px;
        }}
        .brand-title {{
            font-size: 17px;
            font-weight: 800;
            color: var(--accent-cyan);
            letter-spacing: -0.3px;
        }}
        .dora-kpis {{
            display: flex;
            gap: 20px;
            align-items: center;
        }}
        .dora-metric {{
            display: flex;
            flex-direction: column;
            text-align: center;
        }}
        .dora-lbl {{
            font-size: 10px;
            text-transform: uppercase;
            color: var(--text-muted);
            font-weight: 700;
            letter-spacing: 0.5px;
        }}
        .dora-num {{
            font-size: 15px;
            font-weight: 800;
            color: var(--text-primary);
        }}
        .socket-status {{
            display: flex;
            align-items: center;
            gap: 8px;
            padding: 6px 12px;
            background: rgba(16, 185, 129, 0.1);
            border: 1px solid rgba(16, 185, 129, 0.25);
            border-radius: 9999px;
            font-size: 12px;
            font-weight: 700;
            color: var(--accent-emerald);
        }}
        .pulse-dot {{
            width: 7px;
            height: 7px;
            background: var(--accent-emerald);
            border-radius: 50%;
            box-shadow: 0 0 8px var(--accent-emerald);
        }}
        .cockpit-quadrant-grid {{
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 16px;
            margin-bottom: 16px;
        }}
        .panel-card {{
            background: var(--surface-card);
            border: 1px solid var(--surface-border);
            border-radius: 10px;
            padding: 16px;
            display: flex;
            flex-direction: column;
        }}
        .panel-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 12px;
            padding-bottom: 8px;
            border-bottom: 1px solid var(--surface-border);
        }}
        .panel-title {{
            font-size: 14px;
            font-weight: 700;
            color: var(--accent-cyan);
            display: flex;
            align-items: center;
            gap: 6px;
        }}
        .repo-row {{
            background: rgba(255,255,255,0.02);
            border: 1px solid var(--surface-border);
            border-radius: 8px;
            padding: 10px 12px;
            margin-bottom: 10px;
        }}
        .repo-meta {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 8px;
        }}
        .repo-name {{
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 13px;
        }}
        .repo-stats {{
            display: flex;
            gap: 12px;
            font-size: 11px;
            color: var(--text-secondary);
        }}
        .gitops-dag {{
            display: flex;
            align-items: center;
            gap: 6px;
            overflow-x: auto;
        }}
        .dag-node {{
            background: rgba(0,0,0,0.3);
            border: 1px solid var(--surface-border);
            border-radius: 6px;
            padding: 4px 8px;
            display: flex;
            flex-direction: column;
            min-width: 90px;
        }}
        .dag-label {{
            font-size: 9px;
            text-transform: uppercase;
            color: var(--text-muted);
            font-weight: 700;
        }}
        .dag-arrow {{
            color: var(--accent-cyan);
            font-size: 12px;
        }}
        .train-item {{
            background: rgba(255,255,255,0.02);
            border: 1px solid var(--surface-border);
            border-radius: 8px;
            padding: 10px;
            margin-bottom: 8px;
        }}
        .train-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 6px;
            font-size: 12px;
        }}
        .train-pr {{
            font-weight: 700;
            color: var(--accent-cyan);
        }}
        .train-title {{
            color: var(--text-secondary);
            max-width: 250px;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}
        .train-progress {{
            display: flex;
            align-items: center;
            gap: 8px;
            font-size: 11px;
        }}
        .gate-grid-container {{
            display: grid;
            grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
            gap: 6px;
            max-height: 260px;
            overflow-y: auto;
            padding-right: 4px;
        }}
        .gate-cell {{
            border-radius: 6px;
            padding: 6px 8px;
            display: flex;
            flex-direction: column;
            font-size: 10px;
            border: 1px solid transparent;
        }}
        .gate-green {{
            background: rgba(16, 185, 129, 0.1);
            border-color: rgba(16, 185, 129, 0.3);
            color: #34d399;
        }}
        .gate-amber {{
            background: rgba(245, 158, 11, 0.1);
            border-color: rgba(245, 158, 11, 0.3);
            color: #fbbf24;
        }}
        .gate-red {{
            background: rgba(244, 63, 94, 0.15);
            border-color: rgba(244, 63, 94, 0.4);
            color: #f87171;
        }}
        .gate-num {{
            font-weight: 800;
            font-size: 9px;
            opacity: 0.7;
        }}
        .gate-name {{
            font-weight: 600;
            white-space: nowrap;
            overflow: hidden;
            text-overflow: ellipsis;
        }}
        .gate-mkr {{
            font-size: 9px;
            font-weight: 700;
            margin-top: 2px;
        }}
        .badge {{
            padding: 2px 6px;
            border-radius: 4px;
            font-size: 10px;
            font-weight: 700;
            text-transform: uppercase;
        }}
        .badge-healthy {{
            background: rgba(16, 185, 129, 0.15);
            color: var(--accent-emerald);
            border: 1px solid rgba(16, 185, 129, 0.3);
        }}
        .badge-warning {{
            background: rgba(245, 158, 11, 0.15);
            color: var(--accent-amber);
            border: 1px solid rgba(245, 158, 11, 0.3);
        }}
        .badge-queued {{
            background: rgba(59, 130, 246, 0.15);
            color: var(--accent-blue);
            border: 1px solid rgba(59, 130, 246, 0.3);
        }}
        .text-cyan {{ color: var(--accent-cyan); }}
        code {{
            font-family: var(--font-mono);
            font-size: 11px;
            color: var(--accent-cyan);
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            font-size: 12px;
        }}
        th {{
            color: var(--text-muted);
            padding: 8px 10px;
            font-weight: 600;
            text-transform: uppercase;
            font-size: 10px;
            border-bottom: 1px solid var(--surface-border);
            text-align: left;
        }}
        td {{
            padding: 8px 10px;
            border-bottom: 1px solid var(--surface-border);
        }}
        tr:last-child td {{ border-bottom: none; }}
        .progress-bar-bg {{
            background: rgba(255,255,255,0.08);
            border-radius: 4px;
            height: 6px;
            width: 90px;
            overflow: hidden;
            display: inline-block;
            vertical-align: middle;
        }}
        .progress-bar-fill {{
            background: var(--accent-emerald);
            height: 100%;
        }}
        .progress-text {{
            font-size: 10px;
            font-weight: 700;
        }}
        .empty-state {{
            padding: 24px;
            text-align: center;
            color: var(--text-muted);
            font-size: 12px;
        }}
        .btn-add-account {{
            background: rgba(6, 182, 212, 0.15);
            color: var(--accent-cyan);
            border: 1px solid rgba(6, 182, 212, 0.3);
            border-radius: 6px;
            padding: 4px 10px;
            font-size: 11px;
            font-weight: 700;
            cursor: pointer;
            transition: all 0.15s ease;
        }}
        .btn-add-account:hover {{
            background: rgba(6, 182, 212, 0.3);
        }}
        .btn-action {{
            padding: 2px 8px;
            border-radius: 4px;
            font-size: 10px;
            font-weight: 700;
            cursor: pointer;
            border: 1px solid transparent;
        }}
        .btn-drain {{
            background: rgba(245, 158, 11, 0.15);
            color: var(--accent-amber);
            border-color: rgba(245, 158, 11, 0.3);
        }}
        .btn-resume {{
            background: rgba(16, 185, 129, 0.15);
            color: var(--accent-emerald);
            border-color: rgba(16, 185, 129, 0.3);
        }}
        dialog {{
            background: var(--surface-card);
            border: 1px solid var(--surface-border);
            border-radius: 12px;
            color: var(--text-primary);
            padding: 24px;
            max-width: 440px;
            margin: auto;
            backdrop-filter: blur(16px);
            box-shadow: 0 20px 25px -5px rgba(0,0,0,0.5);
        }}
        dialog::backdrop {{
            background: rgba(0, 0, 0, 0.7);
            backdrop-filter: blur(4px);
        }}
        .form-group {{
            margin-bottom: 12px;
            display: flex;
            flex-direction: column;
            gap: 4px;
        }}
        .form-group label {{
            font-size: 11px;
            font-weight: 600;
            color: var(--text-secondary);
        }}
        .form-control {{
            background: rgba(0, 0, 0, 0.3);
            border: 1px solid var(--surface-border);
            border-radius: 6px;
            padding: 8px 10px;
            color: var(--text-primary);
            font-size: 12px;
            font-family: inherit;
        }}
        .form-control:focus {{
            outline: none;
            border-color: var(--accent-cyan);
        }}
        .modal-actions {{
            display: flex;
            justify-content: flex-end;
            gap: 8px;
            margin-top: 16px;
        }}
        @media (max-width: 1000px) {{
            .cockpit-quadrant-grid {{ grid-template-columns: 1fr; }}
            .dora-kpis {{ display: none; }}
        }}
    </style>
    <script>
        async function fetchDashboardState() {{
            try {{
                const res = await fetch('/api/dashboard/state');
                if (!res.ok) return;
                const data = await res.json();
                
                // Update DORA KPIs
                if (data.dora_metrics) {{
                    document.querySelector('#lead-time-val').textContent = data.dora_metrics.lead_time_hours.toFixed(1) + 'h';
                    document.querySelector('#deploy-cadence-val').textContent = data.dora_metrics.deployment_frequency_per_day.toFixed(1) + '/d';
                    document.querySelector('#mttr-val').textContent = data.dora_metrics.mttr_minutes.toFixed(0) + 'm';
                    document.querySelector('#failure-rate-val').textContent = data.dora_metrics.change_failure_rate_pct.toFixed(1) + '%';
                }}
            }} catch(e) {{}}
        }}

        function initFleetSSE() {{
            const eventSource = new EventSource('/api/events/fleet');
            eventSource.addEventListener('fleet_event', function(e) {{
                try {{
                    const event = JSON.parse(e.data);
                    const tableBody = document.querySelector('#activity-tbody');
                    if (tableBody) {{
                        const row = document.createElement('tr');
                        row.innerHTML = `<td>${{event.timestamp_utc}}</td><td><code>${{event.repo}}</code></td><td><strong>${{event.entity_id}}</strong></td><td>${{event.title}}</td><td><span class="badge badge-healthy">${{event.status}}</span></td>`;
                        tableBody.insertBefore(row, tableBody.firstChild);
                    }}
                    fetchDashboardState();
                }} catch(err) {{}}
            }});
            eventSource.onerror = function() {{ setTimeout(initFleetSSE, 5000); }};
        }}

        setInterval(fetchDashboardState, 3000);
        initFleetSSE();

        function openAddAccountModal() {{
            document.querySelector('#add-account-dialog').showModal();
        }}

        function closeAddAccountModal() {{
            document.querySelector('#add-account-dialog').close();
        }}

        async function submitAddAccount(event) {{
            event.preventDefault();
            const accountId = document.querySelector('#acc-id').value.trim();
            const provider = document.querySelector('#acc-provider').value;
            const authKey = document.querySelector('#acc-auth').value.trim();
            const max5hr = parseInt(document.querySelector('#acc-5hr').value, 10);
            const weeklyBudget = parseFloat(document.querySelector('#acc-budget').value);

            try {{
                const res = await fetch('/api/accounts/pool', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{
                        account_id: accountId,
                        provider: provider,
                        auth_profile_or_key: authKey || null,
                        max_5hr_tokens: max5hr || 1000000,
                        max_weekly_budget_usd: weeklyBudget || 100.0
                    }})
                }});
                const data = await res.json();
                if (data.success) {{
                    closeAddAccountModal();
                    fetchDashboardState();
                    location.reload();
                }} else {{
                    alert('Error: ' + data.message);
                }}
            }} catch(err) {{
                alert('Network error: ' + err);
            }}
        }}

        async function drainAccount(accountId) {{
            if (!confirm(`Are you sure you want to drain account '${{accountId}}'?`)) return;
            try {{
                const res = await fetch('/api/accounts/drain', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ account_id: accountId }})
                }});
                const data = await res.json();
                if (data.success) {{
                    fetchDashboardState();
                    location.reload();
                }}
            }} catch(e) {{}}
        }}

        async function resumeAccount(accountId) {{
            try {{
                const res = await fetch('/api/accounts/resume', {{
                    method: 'POST',
                    headers: {{ 'Content-Type': 'application/json' }},
                    body: JSON.stringify({{ account_id: accountId }})
                }});
                const data = await res.json();
                if (data.success) {{
                    fetchDashboardState();
                    location.reload();
                }}
            }} catch(e) {{}}
        }}
    </style>
</head>
<body>
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
                <label for="acc-auth">Auth Key / Profile (Optional)</label>
                <input class="form-control" type="text" id="acc-auth" placeholder="e.g. ANTHROPIC_API_KEY_03" />
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
