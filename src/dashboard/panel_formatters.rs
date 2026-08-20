use crate::dashboard::escape::{html as esc, html_truncated as esc_trunc};
use crate::dashboard::ssr_renderer::DashboardStateView;

pub fn build_repo_cards(state: &DashboardStateView) -> String {
    state
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
                esc(&r.name),
                esc(&r.health_badge),
                r.open_prs,
                r.lead_time_hours,
                r.deploy_frequency_per_day,
                dev_sha, stg_sha, cnr_sha, prd_sha
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn build_merge_train_rows(state: &DashboardStateView) -> String {
    if state.merge_train.is_empty() {
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
                    esc(&t.repo),
                    t.pr_number,
                    esc_trunc(&t.title, 160),
                    esc(&t.state),
                    t.speculative_base, t.head_sha,
                    (t.gates_completed as f64 / t.total_gates.max(1) as f64 * 100.0) as usize,
                    t.gates_completed, t.total_gates
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn build_gate_cells(state: &DashboardStateView) -> String {
    state
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
        .join("\n")
}

pub fn build_account_quota_rows(state: &DashboardStateView) -> String {
    state
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
                    esc(&acc.account_id)
                )
            } else {
                format!(
                    r#"<button class="btn-action btn-drain" onclick="drainAccount('{}')">Drain</button>"#,
                    esc(&acc.account_id)
                )
            };

            let usage_5hr_html = match (acc.pct_5hr_used, acc.remaining_5hr_tokens) {
                (Some(pct), Some(rem)) => format!(
                    r#"<div class="progress-bar-bg"><div class="progress-bar-fill" style="width: {:.1}%"></div></div>
                    <span class="progress-text">{:.1}% ({}k rem)</span>"#,
                    pct, pct, rem / 1000
                ),
                _ => format!(
                    r#"<span class="progress-text">{} tokens • Uncapped CLI</span>"#,
                    acc.used_5hr_tokens
                ),
            };

            let budget_html = match acc.weekly_budget_usd {
                Some(budget) => format!("${:.2} / ${:.2}", acc.weekly_spent_usd, budget),
                None => format!("${:.2} • Uncapped CLI", acc.weekly_spent_usd),
            };

            format!(
                r#"<tr>
                    <td><strong>{}</strong></td>
                    <td><code>{}</code></td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                    <td>{}</td>
                </tr>"#,
                esc(&acc.account_id),
                acc.provider,
                usage_5hr_html,
                budget_html,
                status_badge,
                action_button
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn build_model_rows(state: &DashboardStateView) -> String {
    state
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
        .join("\n")
}

pub fn build_activity_rows(state: &DashboardStateView) -> String {
    state
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
                esc(&act.timestamp),
                esc(&act.repo),
                esc(&act.entity),
                esc_trunc(&act.action, 200),
                esc(&act.status)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
