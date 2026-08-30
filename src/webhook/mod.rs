pub mod admin_auth;
pub mod forwarder_supervisor;
pub mod hmac;
pub mod manual_handlers;
pub mod next_phase;
pub mod pipelines;
pub mod repo_guard;
pub mod sse;
pub mod webhook_handlers;

use axum::{Router, routing::post};
use serde::{Deserialize, Serialize};

use admin_auth::admin_guarded;

pub use manual_handlers::*;
pub use pipelines::{execute_pr_certify, execute_pr_fix, execute_pr_review};
pub use webhook_handlers::webhook_handler;

pub mod state;
pub use state::AppState;

#[derive(Serialize, Deserialize)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

/// Liveness probe handler returning HTTP 200 OK ("ok") for Kubernetes and container orchestration health checks.
pub async fn healthz_handler() -> &'static str {
    "ok"
}

/// Prometheus metrics exposition handler returning standard text format
pub async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    let text = state.metrics.export_prometheus_text();
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        text,
    )
}

/// Constructs the Axum HTTP router with webhook ingress, healthz probes, and on-demand API endpoints.
///
/// # Authentication
///
/// Every `/api/*` route is registered through [`admin_guarded`], which runs the
/// admin check before the handler sees the request. On a loopback bind that
/// check allows everything; on any other bind it requires
/// `X-Anvil-Admin-Token` to match `ANVIL_ADMIN_TOKEN`, and refuses with 403 if
/// no token is configured at all (invariant I1).
///
/// Two routes are deliberately NOT guarded:
///   - `/healthz`, the Kubernetes liveness probe, and `/metrics`, the
///     Prometheus scrape target. Both are pulled by infrastructure that cannot
///     present a token; guarding them makes the pod look unhealthy and gets the
///     whole check disabled.
///   - `/webhook`, which authenticates differently and more strongly: it
///     verifies the GitHub HMAC signature over the request body.
///
/// The dashboard at `/` and `/dashboard` is guarded like the rest. It is an
/// operator surface, not a probe: `dashboard_html_handler` calls the same
/// `fetch_current_dashboard_state` the guarded JSON endpoint does, and renders
/// every watched repository's open pull request titles, branch names and head
/// SHAs into the page. HTML is a rendering, not a category of data.
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route(
            "/",
            axum::routing::get(admin_guarded(crate::dashboard::dashboard_html_handler)),
        )
        .route(
            "/dashboard",
            axum::routing::get(admin_guarded(crate::dashboard::dashboard_html_handler)),
        )
        .route(
            "/api/dashboard/state",
            axum::routing::get(admin_guarded(crate::dashboard::dashboard_state_api_handler)),
        )
        .route(
            "/api/fleet/shape",
            axum::routing::get(admin_guarded(manual_handlers::fleet_shape_handler)),
        )
        .route(
            "/api/events/fleet",
            axum::routing::get(admin_guarded(crate::webhook::sse::sse_fleet_stream_handler)),
        )
        .route("/healthz", axum::routing::get(healthz_handler))
        .route("/metrics", axum::routing::get(metrics_handler))
        .route("/webhook", post(webhook_handler))
        .route("/api/review", post(admin_guarded(manual_review_handler)))
        .route("/api/fix", post(admin_guarded(manual_fix_handler)))
        .route("/api/certify", post(admin_guarded(manual_certify_handler)))
        .route("/api/triage", post(admin_guarded(manual_triage_handler)))
        .route("/api/enlist", post(admin_guarded(manual_enlist_handler)))
        .route(
            "/api/heal-queue",
            post(admin_guarded(manual_heal_queue_handler)),
        )
        .route(
            "/api/reconcile",
            post(admin_guarded(manual_reconcile_handler)),
        )
        .route(
            "/api/drain",
            post(admin_guarded(manual_handlers::drain_handler)),
        )
        .route(
            "/api/accounts/pool",
            post(admin_guarded(manual_handlers::add_account_pool_handler)),
        )
        .route(
            "/api/accounts/drain",
            post(admin_guarded(manual_handlers::drain_account_handler)),
        )
        .route(
            "/api/accounts/resume",
            post(admin_guarded(manual_handlers::resume_account_handler)),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_healthz_handler() {
        let resp = healthz_handler().await;
        assert_eq!(resp, "ok");
    }
}
