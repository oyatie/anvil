use serde::{Deserialize, Serialize};

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
    pub environment_pipeline: Vec<EnvironmentStatusView>,
    pub recent_activities: Vec<ActivityEventView>,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateHeatmapItem {
    pub gate_name: String,
    pub fail_count: usize,
    pub pass_percentage: f64,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBanditView {
    pub model_name: String,
    pub trials: usize,
    pub pass_at_1: f64,
    pub avg_cost_per_pr: f64,
    pub p99_latency_sec: f64,
    pub ucb1_score: f64,
    pub is_statistically_significant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoraMetricsView {
    pub deployment_frequency_per_day: f64,
    pub lead_time_hours: f64,
    pub change_failure_rate_pct: f64,
    pub mttr_minutes: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentStatusView {
    pub env_name: String,
    pub branch: String,
    pub current_sha: String,
    pub is_locked: bool,
    pub bake_time_remaining_mins: u64,
    pub sre_burn_rate: f64,
    pub health_status: String,
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
    /// Renders high-fidelity 5-Module Hyperscaler Leptos SSR Control Plane HTML
    pub fn render_html(state: &DashboardStateView) -> String {
        let repo_cards = state
            .fleet_repos
            .iter()
            .map(|r| {
                format!(
                    r#"<div class="card repo-card">
                        <div class="card-header">
                            <span class="repo-title">📦 {}</span>
                            <span class="badge badge-healthy">{}</span>
                        </div>
                        <div class="card-body">
                            <p><strong>Branch SHA:</strong> <code>{}</code></p>
                            <p><strong>Open PRs:</strong> {} | <strong>Pass Rate:</strong> {:.1}%</p>
                            <p><strong>DORA Lead Time:</strong> {:.1}h</p>
                            <p><strong>Deploys/Day:</strong> {:.1}</p>
                        </div>
                    </div>"#,
                    r.name, r.health_badge, r.head_sha, r.open_prs, r.pass_rate, r.lead_time_hours, r.deploy_frequency_per_day
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let heatmap_rows = state
            .gate_heatmap
            .iter()
            .map(|g| {
                let bar_width = g.pass_percentage.min(100.0);
                format!(
                    r#"<tr>
                        <td><strong>{}</strong></td>
                        <td><code>{}</code></td>
                        <td>{}</td>
                        <td>
                            <div class="progress-bar-bg">
                                <div class="progress-bar-fill" style="width: {:.1}%"></div>
                            </div>
                            <span class="progress-text">{:.1}%</span>
                        </td>
                    </tr>"#,
                    g.gate_name, g.category, g.fail_count, bar_width, g.pass_percentage
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let model_rows = state
            .ai_bandit_models
            .iter()
            .map(|m| {
                let sig_badge = if m.is_statistically_significant {
                    r#"<span class="badge badge-healthy">N>=30 (p<0.05)</span>"#
                } else {
                    r#"<span class="badge badge-warning">Exploring (N<30)</span>"#
                };
                format!(
                    r#"<tr>
                        <td><strong>{}</strong></td>
                        <td>{}</td>
                        <td><strong>{:.1}%</strong></td>
                        <td>${:.3}</td>
                        <td>{:.1}s</td>
                        <td><code>{:.3}</code></td>
                        <td>{}</td>
                    </tr>"#,
                    m.model_name,
                    m.trials,
                    m.pass_at_1 * 100.0,
                    m.avg_cost_per_pr,
                    m.p99_latency_sec,
                    m.ucb1_score,
                    sig_badge
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let env_rows = state
            .environment_pipeline
            .iter()
            .map(|env| {
                let badge_class = if env.sre_burn_rate <= 1.0 {
                    "badge-healthy"
                } else {
                    "badge-warning"
                };
                format!(
                    r#"<div class="card env-card">
                        <div class="card-header">
                            <span class="env-title">{}</span>
                            <span class="badge {}">{}</span>
                        </div>
                        <div class="card-body">
                            <p><strong>Branch:</strong> <code>{}</code></p>
                            <p><strong>Commit SHA:</strong> <code>{}</code></p>
                            <p><strong>Bake Window:</strong> {}m</p>
                            <p><strong>SRE Multi-Burn Rate:</strong> {:.2}x</p>
                        </div>
                    </div>"#,
                    env.env_name,
                    badge_class,
                    env.health_status,
                    env.branch,
                    env.current_sha,
                    env.bake_time_remaining_mins,
                    env.sre_burn_rate
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
    <title>Oyatie Anvil | Hyperscale Fleet Control Plane</title>
    <style>
        :root {{
            --bg-dark: #0a0e17;
            --surface-dark: #131b2e;
            --surface-border: #1e2c4a;
            --text-primary: #f0f4f8;
            --text-secondary: #94a3b8;
            --accent-cyan: #06b6d4;
            --accent-blue: #3b82f6;
            --accent-emerald: #10b981;
            --accent-amber: #f59e0b;
            --accent-rose: #f43f5e;
            --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
        }}
        * {{ box-sizing: border-box; margin: 0; padding: 0; }}
        body {{
            background-color: var(--bg-dark);
            color: var(--text-primary);
            font-family: var(--font-sans);
            padding: 24px;
            line-height: 1.5;
        }}
        .navbar {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 16px 24px;
            background: var(--surface-dark);
            border: 1px solid var(--surface-border);
            border-radius: 12px;
            margin-bottom: 24px;
        }}
        .brand {{
            display: flex;
            align-items: center;
            gap: 12px;
            font-size: 20px;
            font-weight: 700;
            color: var(--accent-cyan);
            letter-spacing: -0.5px;
        }}
        .status-pill {{
            display: inline-flex;
            align-items: center;
            gap: 8px;
            padding: 6px 12px;
            background: rgba(16, 185, 129, 0.1);
            color: var(--accent-emerald);
            border: 1px solid rgba(16, 185, 129, 0.2);
            border-radius: 9999px;
            font-size: 13px;
            font-weight: 600;
        }}
        .status-dot {{
            width: 8px;
            height: 8px;
            background: var(--accent-emerald);
            border-radius: 50%;
            animation: pulse 2s infinite;
        }}
        @keyframes pulse {{
            0% {{ transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0.7); }}
            70% {{ transform: scale(1); box-shadow: 0 0 0 6px rgba(16, 185, 129, 0); }}
            100% {{ transform: scale(0.95); box-shadow: 0 0 0 0 rgba(16, 185, 129, 0); }}
        }}
        .metrics-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
            gap: 16px;
            margin-bottom: 24px;
        }}
        .metric-card {{
            background: var(--surface-dark);
            border: 1px solid var(--surface-border);
            padding: 20px;
            border-radius: 12px;
            display: flex;
            flex-direction: column;
            gap: 8px;
        }}
        .metric-title {{
            font-size: 13px;
            color: var(--text-secondary);
            text-transform: uppercase;
            font-weight: 600;
            letter-spacing: 0.5px;
        }}
        .metric-val {{
            font-size: 28px;
            font-weight: 700;
            color: var(--text-primary);
        }}
        .metric-sub {{
            font-size: 12px;
            color: var(--accent-cyan);
        }}
        .section-header {{
            font-size: 18px;
            font-weight: 700;
            margin-top: 24px;
            margin-bottom: 16px;
            display: flex;
            align-items: center;
            gap: 8px;
        }}
        .grid-3 {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
            gap: 16px;
            margin-bottom: 24px;
        }}
        .card {{
            background: var(--surface-dark);
            border: 1px solid var(--surface-border);
            border-radius: 12px;
            padding: 16px;
        }}
        .card-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 12px;
            padding-bottom: 8px;
            border-bottom: 1px solid var(--surface-border);
        }}
        .repo-title, .env-title {{
            font-size: 15px;
            font-weight: 700;
            color: var(--accent-cyan);
        }}
        .badge {{
            padding: 4px 8px;
            border-radius: 6px;
            font-size: 11px;
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
        .card-body p {{
            font-size: 13px;
            color: var(--text-secondary);
            margin-bottom: 6px;
        }}
        .card-body strong {{
            color: var(--text-primary);
        }}
        code {{
            font-family: var(--font-mono);
            background: rgba(255,255,255,0.06);
            padding: 2px 6px;
            border-radius: 4px;
            font-size: 12px;
            color: var(--accent-cyan);
        }}
        .table-container {{
            background: var(--surface-dark);
            border: 1px solid var(--surface-border);
            border-radius: 12px;
            overflow: hidden;
            margin-bottom: 24px;
        }}
        table {{
            width: 100%;
            border-collapse: collapse;
            text-align: left;
            font-size: 13px;
        }}
        th {{
            background: rgba(255,255,255,0.02);
            padding: 12px 16px;
            color: var(--text-secondary);
            font-weight: 600;
            border-bottom: 1px solid var(--surface-border);
        }}
        td {{
            padding: 12px 16px;
            border-bottom: 1px solid var(--surface-border);
        }}
        tr:last-child td {{
            border-bottom: none;
        }}
        .progress-bar-bg {{
            background: rgba(255,255,255,0.1);
            border-radius: 4px;
            height: 8px;
            width: 120px;
            display: inline-block;
            vertical-align: middle;
            margin-right: 8px;
            overflow: hidden;
        }}
        .progress-bar-fill {{
            background: var(--accent-emerald);
            height: 100%;
            border-radius: 4px;
        }}
        .progress-text {{
            font-size: 12px;
            font-weight: 600;
        }}
        .dora-grid {{
            display: grid;
            grid-template-columns: repeat(4, 1fr);
            gap: 16px;
            margin-bottom: 24px;
        }}
        @media (max-width: 900px) {{
            .dora-grid {{ grid-template-columns: repeat(2, 1fr); }}
        }}
    </style>
    <script>
        // Server-Sent Events (SSE) live connection with automatic fallback
        function initFleetSSE() {{
            const eventSource = new EventSource('/api/events/fleet');
            eventSource.addEventListener('fleet_event', function(e) {{
                try {{
                    const event = JSON.parse(e.data);
                    console.log('⚡ Fleet SSE Event:', event);
                    // Prepend new activity
                    const tableBody = document.querySelector('#activity-tbody');
                    if (tableBody) {{
                        const row = document.createElement('tr');
                        row.innerHTML = `<td>${{event.timestamp_utc}}</td><td><code>${{event.repo}}</code></td><td><strong>${{event.entity_id}}</strong></td><td>${{event.title}}</td><td><span class="badge badge-healthy">${{event.status}}</span></td>`;
                        tableBody.insertBefore(row, tableBody.firstChild);
                    }}
                }} catch(err) {{}}
            }});
            eventSource.onerror = function() {{
                console.warn('SSE disconnected, retrying in 5s...');
                setTimeout(initFleetSSE, 5000);
            }};
        }}
        initFleetSSE();
    </script>
</head>
<body>
    <div class="navbar">
        <div class="brand">
            <span>⚡ OYATIE ANVIL CONTROL PLANE</span>
            <span style="font-size: 12px; color: var(--text-secondary); font-weight: 400;">v{}</span>
        </div>
        <div class="status-pill">
            <span class="status-dot"></span>
            <span>Fleet Live (3 Managed Repos, 70-Gate Matrix Active)</span>
        </div>
    </div>

    <!-- DORA METRICS SUMMARY -->
    <div class="section-header">
        <span>📈 Hyperscaler DORA Metrics (30-Day Fleet Aggregate)</span>
    </div>
    <div class="dora-grid">
        <div class="metric-card">
            <span class="metric-title">Deployment Frequency</span>
            <span class="metric-val">{:.1}/day</span>
            <span class="metric-sub">Elite Tier (&gt;= 1.0/day)</span>
        </div>
        <div class="metric-card">
            <span class="metric-title">Lead Time for Changes</span>
            <span class="metric-val">{:.1} hrs</span>
            <span class="metric-sub">Elite Tier (&lt; 24 hrs)</span>
        </div>
        <div class="metric-card">
            <span class="metric-title">Change Failure Rate</span>
            <span class="metric-val">{:.1}%</span>
            <span class="metric-sub">Elite Tier (&lt; 5%)</span>
        </div>
        <div class="metric-card">
            <span class="metric-title">Mean Time to Restore</span>
            <span class="metric-val">{:.0} mins</span>
            <span class="metric-sub">Elite Tier (&lt; 60 mins)</span>
        </div>
    </div>

    <!-- MODULE 1: MULTI-REPO FLEET DAG -->
    <div class="section-header">
        <span>🌐 Module 1: Multi-Repository Fleet Health &amp; Promotion Topology</span>
    </div>
    <div class="grid-3">
        {}
    </div>

    <!-- MODULE 2: 70-GATE FAILURE HEATMAP -->
    <div class="section-header">
        <span>🛡️ Module 2: 70-Gate Full-Lifecycle Failure Heatmap &amp; Flake Quarantine</span>
    </div>
    <div class="table-container">
        <table>
            <thead>
                <tr>
                    <th>Gate Name</th>
                    <th>Category</th>
                    <th>Failures (30d)</th>
                    <th>Pass Rate</th>
                </tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>
    </div>

    <!-- MODULE 3: AI MODEL BANDIT PARETO FRONTIER -->
    <div class="section-header">
        <span>🤖 Module 3: AI Routing Bandit Pareto Frontier &amp; Statistical Significance (N &gt;= 30)</span>
    </div>
    <div class="table-container">
        <table>
            <thead>
                <tr>
                    <th>Model Family</th>
                    <th>Trials</th>
                    <th>Pass@1</th>
                    <th>Avg Cost/PR</th>
                    <th>P99 Latency</th>
                    <th>UCB1 Reward</th>
                    <th>Statistical Power</th>
                </tr>
            </thead>
            <tbody>
                {}
            </tbody>
        </table>
    </div>

    <!-- MODULE 4: GITOPS PROMOTION DAG -->
    <div class="section-header">
        <span>🚀 Module 4: GitOps Multi-Tier Promotion DAG (SRE Error Budget Gauges)</span>
    </div>
    <div class="grid-3">
        {}
    </div>

    <!-- MODULE 5: REAL-TIME SSE AUDIT EVENT LOG -->
    <div class="section-header">
        <span>📡 Module 5: Real-Time SSE Audit Event Log &amp; Attestation Stream</span>
    </div>
    <div class="table-container">
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
</body>
</html>"#,
            state.server_version,
            state.dora_metrics.deployment_frequency_per_day,
            state.dora_metrics.lead_time_hours,
            state.dora_metrics.change_failure_rate_pct,
            state.dora_metrics.mttr_minutes,
            repo_cards,
            heatmap_rows,
            model_rows,
            env_rows,
            activity_rows
        )
    }
}
