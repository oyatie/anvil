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
    pub environment_pipeline: Vec<EnvironmentStatusView>,
    pub recent_activities: Vec<ActivityEventView>,
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
    /// Renders high-fidelity Leptos-style Server-Side Rendered (SSR) HTML dashboard
    pub fn render_html(state: &DashboardStateView) -> String {
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
                    r#"<div class="env-card">
                        <div class="env-header">
                            <span class="env-title">{}</span>
                            <span class="badge {}">{}</span>
                        </div>
                        <div class="env-body">
                            <p><strong>Branch:</strong> <code>{}</code></p>
                            <p><strong>Commit SHA:</strong> <code>{}</code></p>
                            <p><strong>Bake Remaining:</strong> {}m</p>
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
    <title>Oyatie Anvil | Hyperscale Delivery Fabric</title>
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
            margin-bottom: 16px;
            display: flex;
            align-items: center;
            gap: 8px;
        }}
        .pipeline-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
            gap: 16px;
            margin-bottom: 24px;
        }}
        .env-card {{
            background: var(--surface-dark);
            border: 1px solid var(--surface-border);
            border-radius: 12px;
            padding: 16px;
        }}
        .env-header {{
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 12px;
            padding-bottom: 8px;
            border-bottom: 1px solid var(--surface-border);
        }}
        .env-title {{
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
        .env-body p {{
            font-size: 13px;
            color: var(--text-secondary);
            margin-bottom: 6px;
        }}
        .env-body strong {{
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
    </style>
    <script>
        // Auto-refresh telemetry every 5 seconds
        setInterval(async () => {{
            try {{
                const res = await fetch('/api/dashboard/state');
                if (res.ok) {{
                    const data = await res.json();
                    document.getElementById('metric-pass1').innerText = (data.compiler_pass_at_1_ratio * 100).toFixed(1) + '%';
                    document.getElementById('metric-quota').innerText = '$' + data.quota_spent_usd.toFixed(2) + ' / $' + data.quota_budget_usd.toFixed(0);
                    document.getElementById('metric-processes').innerText = data.active_processes_count;
                }}
            }} catch (e) {{}}
        }}, 5000);
    </script>
</head>
<body>
    <div class="navbar">
        <div class="brand">
            <span>⚡ OYATIE ANVIL</span>
            <span style="font-size: 12px; color: var(--text-secondary); font-weight: 400;">v{}</span>
        </div>
        <div class="status-pill">
            <span class="status-dot"></span>
            <span>Autonomous Self-Governor Live (70-Gate Matrix Active)</span>
        </div>
    </div>

    <div class="metrics-grid">
        <div class="metric-card">
            <span class="metric-title">Pass@1 Compiler Rate</span>
            <span class="metric-val" id="metric-pass1">{:.1}%</span>
            <span class="metric-sub">Closed-Loop Bandit SLA &gt;= 90%</span>
        </div>
        <div class="metric-card">
            <span class="metric-title">LLM Quota Budget Spend</span>
            <span class="metric-val" id="metric-quota">${:.2} / ${:.0}</span>
            <span class="metric-sub">Circuit Breaker Active</span>
        </div>
        <div class="metric-card">
            <span class="metric-title">Active Process Registry</span>
            <span class="metric-val" id="metric-processes">{}</span>
            <span class="metric-sub">Sliding Inactivity SLA &lt;= 30s</span>
        </div>
        <div class="metric-card">
            <span class="metric-title">16-Lens Quality Index</span>
            <span class="metric-val">{:.2} / 1.0</span>
            <span class="metric-sub">Zero-Trust &amp; ADR Parity</span>
        </div>
    </div>

    <div class="section-header">
        <span>🚀 Multi-Tier GitOps Promotion Pipeline (DAG)</span>
    </div>
    <div class="pipeline-grid">
        {}
    </div>

    <div class="section-header">
        <span>📋 Autonomous Activity &amp; Reconciliation Log</span>
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
            <tbody>
                {}
            </tbody>
        </table>
    </div>
</body>
</html>"#,
            state.server_version,
            state.compiler_pass_at_1_ratio * 100.0,
            state.quota_spent_usd,
            state.quota_budget_usd,
            state.active_processes_count,
            state.quality_score_mean,
            env_rows,
            activity_rows
        )
    }
}
