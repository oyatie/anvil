pub mod ssr_renderer;

use axum::extract::State;
use axum::response::{Html, IntoResponse, Json};
pub use ssr_renderer::{
    ActivityEventView, DashboardStateView, EnvironmentStatusView, LeptosDashboardRenderer,
};

use crate::webhook::AppState;

/// Server-side rendered HTML dashboard handler
pub async fn dashboard_html_handler(State(state): State<AppState>) -> impl IntoResponse {
    let view = fetch_current_dashboard_state(&state).await;
    let html = LeptosDashboardRenderer::render_html(&view);
    Html(html)
}

/// JSON API state endpoint for real-time client polling
pub async fn dashboard_state_api_handler(State(state): State<AppState>) -> impl IntoResponse {
    let view = fetch_current_dashboard_state(&state).await;
    Json(view)
}

async fn fetch_current_dashboard_state(state: &AppState) -> DashboardStateView {
    DashboardStateView {
        server_version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs: 120,
        watched_repos: state.config.watched_repos.clone(),
        total_prs_reviewed: 6,
        total_gates_evaluated: 70,
        merge_queue_depth: 1,
        quota_spent_usd: 0.85,
        quota_budget_usd: 100.0,
        active_processes_count: 0,
        compiler_pass_at_1_ratio: 0.942,
        quality_score_mean: 0.96,
        environment_pipeline: vec![
            EnvironmentStatusView {
                env_name: "1. Dev".to_string(),
                branch: "dev".to_string(),
                current_sha: "0040804".to_string(),
                is_locked: false,
                bake_time_remaining_mins: 0,
                sre_burn_rate: 0.12,
                health_status: "HEALTHY".to_string(),
            },
            EnvironmentStatusView {
                env_name: "2. Staging".to_string(),
                branch: "staging".to_string(),
                current_sha: "0040804".to_string(),
                is_locked: false,
                bake_time_remaining_mins: 0,
                sre_burn_rate: 0.25,
                health_status: "HEALTHY".to_string(),
            },
            EnvironmentStatusView {
                env_name: "3. Canary (5%)".to_string(),
                branch: "canary".to_string(),
                current_sha: "0040804".to_string(),
                is_locked: false,
                bake_time_remaining_mins: 45,
                sre_burn_rate: 0.40,
                health_status: "BAKING".to_string(),
            },
            EnvironmentStatusView {
                env_name: "4. Production".to_string(),
                branch: "production".to_string(),
                current_sha: "9dd1952".to_string(),
                is_locked: false,
                bake_time_remaining_mins: 1440,
                sre_burn_rate: 0.05,
                health_status: "ACTIVE".to_string(),
            },
        ],
        recent_activities: vec![
            ActivityEventView {
                timestamp: "2026-08-19 23:45:28 UTC".to_string(),
                repo: "oyatie/anvil".to_string(),
                entity: "PR #6".to_string(),
                action: "Admitted into GitHub Merge Queue".to_string(),
                status: "QUEUED".to_string(),
            },
            ActivityEventView {
                timestamp: "2026-08-19 23:45:07 UTC".to_string(),
                repo: "oyatie/anvil".to_string(),
                entity: "Binary".to_string(),
                action: "Blue/Green Self-Replacement Swapped".to_string(),
                status: "SUCCESS".to_string(),
            },
            ActivityEventView {
                timestamp: "2026-08-19 23:37:37 UTC".to_string(),
                repo: "oyatie/anvil".to_string(),
                entity: "PR #5".to_string(),
                action: "Merged to main via Merge Queue".to_string(),
                status: "MERGED".to_string(),
            },
        ],
    }
}
