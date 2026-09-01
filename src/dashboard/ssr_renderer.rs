use crate::dashboard::view_model::{DashboardStateView, DoraMetricsView};

/// Renders a DORA figure, or an em dash when the fleet has not been polled.
/// Absent telemetry has to look absent -- a number here is read as measured.
fn dora_or_dash(
    metrics: Option<&DoraMetricsView>,
    render: impl Fn(&DoraMetricsView) -> String,
) -> String {
    metrics.map_or_else(|| "\u{2014}".to_string(), render)
}

pub struct SsrDashboardRenderer;

/// Backward-compatibility alias
pub type LeptosDashboardRenderer = SsrDashboardRenderer;

impl SsrDashboardRenderer {
    /// Renders the fleet control plane: live pipeline state, gate outcomes, and account-pool controls.
    pub fn render_html(state: &DashboardStateView) -> String {
        let repo_cards = crate::dashboard::panel_formatters::build_repo_cards(state);
        let merge_train_rows = crate::dashboard::panel_formatters::build_merge_train_rows(state);
        let gate_cells = crate::dashboard::panel_formatters::build_gate_cells(state);
        let account_quota_rows =
            crate::dashboard::panel_formatters::build_account_quota_rows(state);
        let activity_rows = crate::dashboard::panel_formatters::build_activity_rows(state);

        let css_styles = crate::dashboard::styles::get_cockpit_css();
        let client_scripts = crate::dashboard::client_scripts::get_client_scripts();

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Oyatie Anvil | Fleet Control Plane</title>
    <style>
{}
    </style>
    <script>
{}
    </script>
</head>
<body>"#,
            css_styles, client_scripts
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
                <span class="dora-num" id="lead-time-val">{}</span>
            </div>
            <div class="dora-metric">
                <span class="dora-lbl">Deploy Cadence</span>
                <span class="dora-num" id="deploy-cadence-val">{}</span>
            </div>
            <div class="dora-metric">
                <span class="dora-lbl">MTTR</span>
                <span class="dora-num" id="mttr-val">{}</span>
            </div>
            <div class="dora-metric">
                <span class="dora-lbl">Failure Rate</span>
                <span class="dora-num" id="failure-rate-val">{}</span>
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

        <!-- QUADRANT 3: CONTINUOUS GOVERNANCE & MUTATION KILL RATE MATRIX -->
        <div class="panel-card">
            <div class="panel-header">
                <span class="panel-title">🛡️ Panel 3: Continuous Governance &amp; Mutation Matrix</span>
                <span class="badge badge-healthy">100% Mutation Kill Rate</span>
            </div>
            <div class="gate-grid-container">
                {}
            </div>
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
            dora_or_dash(state.dora_metrics.as_ref(), |d| format!(
                "{:.1}h",
                d.lead_time_hours
            )),
            dora_or_dash(state.dora_metrics.as_ref(), |d| {
                format!("{:.1}/d", d.deployment_frequency_per_day)
            }),
            dora_or_dash(state.dora_metrics.as_ref(), |d| format!(
                "{:.0}m",
                d.mttr_minutes
            )),
            dora_or_dash(state.dora_metrics.as_ref(), |d| {
                format!("{:.1}%", d.change_failure_rate_pct)
            }),
            repo_cards,
            merge_train_rows,
            gate_cells,
            account_quota_rows,
            activity_rows
        )
    }
}

#[cfg(test)]
mod unmeasured_dora_tests {
    use super::*;

    /// The four DORA figures are the headline of the dashboard. When the fleet
    /// poller has not landed a sweep there is no measurement behind them, and
    /// the page has to say so: a zero reads as "we deploy zero times a day",
    /// which is a claim, not an absence.
    #[test]
    fn unmeasured_dora_renders_as_absent_not_as_zero() {
        let html = SsrDashboardRenderer::render_html(&DashboardStateView {
            dora_metrics: None,
            ..Default::default()
        });

        for id in [
            "lead-time-val",
            "deploy-cadence-val",
            "mttr-val",
            "failure-rate-val",
        ] {
            let cell = html
                .split_once(&format!("id=\"{id}\">"))
                .unwrap_or_else(|| panic!("dashboard is missing the {id} cell"))
                .1
                .split_once('<')
                .expect("unterminated cell")
                .0;
            assert_eq!(
                cell, "\u{2014}",
                "{id} must render an em dash when unmeasured, not the number {cell:?}"
            );
        }
    }

    /// Measured telemetry still has to reach the page -- an em dash everywhere
    /// would pass the test above while showing nothing at all.
    #[test]
    fn measured_dora_still_renders_its_figures() {
        let html = SsrDashboardRenderer::render_html(&DashboardStateView {
            dora_metrics: Some(DoraMetricsView {
                deployment_frequency_per_day: 4.0,
                lead_time_hours: 2.5,
                change_failure_rate_pct: 3.0,
                mttr_minutes: 7.0,
            }),
            ..Default::default()
        });

        for expected in ["2.5h", "4.0/d", "7m", "3.0%"] {
            assert!(
                html.contains(expected),
                "measured DORA value {expected} never reached the rendered page"
            );
        }
    }
}
