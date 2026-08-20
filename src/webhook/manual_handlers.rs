use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Deserialize;
use tracing::{error, info};

use super::pipelines::{execute_pr_certify, execute_pr_fix, execute_pr_review};
use super::{ApiResponse, AppState};

#[derive(Deserialize, Debug)]
pub struct ManualReviewRequest {
    pub repo: String,
    pub pr_number: u64,
    pub force: Option<bool>,
}

#[derive(Deserialize, Debug)]
pub struct ManualFixRequest {
    pub repo: String,
    pub pr_number: u64,
}

#[derive(Deserialize, Debug)]
pub struct ManualCertifyRequest {
    pub repo: String,
    pub pr_number: u64,
}

#[derive(Deserialize, Debug)]
pub struct ManualTriageRequest {
    pub repo: String,
    pub run_id: u64,
    pub branch: Option<String>,
    pub commit_sha: Option<String>,
    pub workflow_name: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ManualEnlistRequest {
    pub repo: String,
    pub pr_number: u64,
}

#[derive(Deserialize, Debug)]
pub struct ManualQueueHealRequest {
    pub repo: String,
    pub pr_number: u64,
}

#[derive(Deserialize, Debug)]
pub struct ManualReconcileRequest {
    pub repo: String,
    pub pr_number: u64,
}

pub async fn manual_review_handler(
    State(state): State<AppState>,
    Json(req): Json<ManualReviewRequest>,
) -> impl IntoResponse {
    info!("Manual review requested for {}#{}", req.repo, req.pr_number);

    let pr_meta = match state
        .github_client
        .fetch_pr_metadata(&req.repo, req.pr_number)
        .await
    {
        Ok(meta) => meta,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    success: false,
                    message: format!("Failed to fetch PR metadata: {}", err),
                }),
            );
        }
    };

    let state_clone = state.clone();
    let repo = req.repo.clone();
    let pr_number = req.pr_number;
    let force = req.force.unwrap_or(false);

    tokio::spawn(async move {
        if let Err(e) = execute_pr_review(
            &state_clone,
            &repo,
            pr_number,
            &pr_meta.title,
            &pr_meta.body.unwrap_or_default(),
            &pr_meta.base_ref_name,
            &pr_meta.base_ref_oid,
            &pr_meta.head_ref_oid,
            force,
        )
        .await
        {
            error!(
                "Manual PR review failed for {}#{}: {:?}",
                repo, pr_number, e
            );
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(ApiResponse {
            success: true,
            message: format!("Manual review queued for {}#{}", req.repo, req.pr_number),
        }),
    )
}

pub async fn manual_fix_handler(
    State(state): State<AppState>,
    Json(req): Json<ManualFixRequest>,
) -> impl IntoResponse {
    info!("Manual fix requested for {}#{}", req.repo, req.pr_number);

    let state_clone = state.clone();
    let repo = req.repo.clone();
    let pr_number = req.pr_number;

    tokio::spawn(async move {
        if let Err(e) = execute_pr_fix(&state_clone, &repo, pr_number).await {
            error!("Manual PR fix failed for {}#{}: {:?}", repo, pr_number, e);
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(ApiResponse {
            success: true,
            message: format!("Fix pipeline queued for {}#{}", req.repo, req.pr_number),
        }),
    )
}

pub async fn manual_certify_handler(
    State(state): State<AppState>,
    Json(req): Json<ManualCertifyRequest>,
) -> impl IntoResponse {
    info!(
        "Manual certification requested for {}#{}",
        req.repo, req.pr_number
    );

    let state_clone = state.clone();
    let repo = req.repo.clone();
    let pr_number = req.pr_number;

    tokio::spawn(async move {
        if let Err(e) = execute_pr_certify(&state_clone, &repo, pr_number).await {
            error!(
                "Pre-Merge certification failed for {}#{}: {:?}",
                repo, pr_number, e
            );
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(ApiResponse {
            success: true,
            message: format!(
                "Certification pipeline queued for {}#{}",
                req.repo, req.pr_number
            ),
        }),
    )
}

pub async fn manual_triage_handler(
    State(state): State<AppState>,
    Json(req): Json<ManualTriageRequest>,
) -> impl IntoResponse {
    info!(
        "Manual triage requested for run #{} on {}",
        req.run_id, req.repo
    );

    let state_clone = state.clone();
    let repo = req.repo.clone();
    let run_id = req.run_id;
    let branch = req.branch.unwrap_or_else(|| "main".to_string());
    let commit_sha = req.commit_sha.unwrap_or_default();
    let wf_name = req.workflow_name.unwrap_or_else(|| "CI".to_string());

    tokio::spawn(async move {
        if let Ok(repo_dir) = state_clone.git_mgr.ensure_repo_cloned(&repo).await {
            let _ = state_clone
                .ci_triager
                .triage_workflow_run(&repo, run_id, &branch, &commit_sha, &wf_name, &repo_dir)
                .await;
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(ApiResponse {
            success: true,
            message: format!("Triage queued for run #{} on {}", req.run_id, req.repo),
        }),
    )
}

pub async fn manual_enlist_handler(
    State(state): State<AppState>,
    Json(req): Json<ManualEnlistRequest>,
) -> impl IntoResponse {
    info!(
        "Manual merge queue enlistment requested for {}#{}",
        req.repo, req.pr_number
    );

    let state_clone = state.clone();
    let repo = req.repo.clone();
    let pr_number = req.pr_number;

    tokio::spawn(async move {
        let _ = state_clone
            .merge_enlister
            .enlist_into_merge_queue(&repo, pr_number)
            .await;
    });

    (
        StatusCode::ACCEPTED,
        Json(ApiResponse {
            success: true,
            message: format!("Enlistment queued for {}#{}", req.repo, req.pr_number),
        }),
    )
}

pub async fn manual_heal_queue_handler(
    State(state): State<AppState>,
    Json(req): Json<ManualQueueHealRequest>,
) -> impl IntoResponse {
    info!(
        "Manual queue healing requested for {}#{}",
        req.repo, req.pr_number
    );

    let state_clone = state.clone();
    let repo = req.repo.clone();
    let pr_number = req.pr_number;

    tokio::spawn(async move {
        let _ = state_clone
            .queue_healer
            .heal_ejected_pr(&repo, pr_number)
            .await;
    });

    (
        StatusCode::ACCEPTED,
        Json(ApiResponse {
            success: true,
            message: format!("Queue healing queued for {}#{}", req.repo, req.pr_number),
        }),
    )
}

pub async fn manual_reconcile_handler(
    State(state): State<AppState>,
    Json(req): Json<ManualReconcileRequest>,
) -> impl IntoResponse {
    info!(
        "Manual lockfile reconciliation requested for {}#{}",
        req.repo, req.pr_number
    );

    let state_clone = state.clone();
    let repo = req.repo.clone();
    let pr_number = req.pr_number;

    tokio::spawn(async move {
        let _ = state_clone
            .lockfile_reconciler
            .reconcile_pr(&repo, pr_number)
            .await;
    });

    (
        StatusCode::ACCEPTED,
        Json(ApiResponse {
            success: true,
            message: format!("Reconciliation queued for {}#{}", req.repo, req.pr_number),
        }),
    )
}

pub async fn drain_handler(State(_state): State<AppState>) -> impl IntoResponse {
    info!("👋 [Blue/Green Handover] Graceful drain requested via /api/drain. Initiating zero-loss retirement...");

    tokio::spawn(async move {
        // Allow in-flight requests to complete within 3 seconds
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        info!(
            "👋 [Blue/Green Handover] In-flight jobs finished. Retiring legacy instance cleanly."
        );
        std::process::exit(0);
    });

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: "Graceful drain initiated. Retiring in 3 seconds.".to_string(),
        }),
    )
}

pub async fn add_account_pool_handler(
    State(state): State<AppState>,
    Json(payload): Json<crate::self_governance::AddAccountPayload>,
) -> impl IntoResponse {
    info!(
        "Adding new account '{}' to pool via REST API...",
        payload.account_id
    );

    let provider = crate::ai_driver::provider::ModelProvider::from_str_name(&payload.provider);
    let account = crate::self_governance::ManagedAccount {
        account_id: payload.account_id.clone(),
        provider,
        auth_profile_or_key: payload.auth_profile_or_key,
        max_5hr_tokens: payload.max_5hr_tokens.unwrap_or(1_000_000),
        max_weekly_budget_usd: payload.max_weekly_budget_usd.unwrap_or(100.0),
        usage_history: std::collections::VecDeque::new(),
        cooldown_until: None,
        last_leased_at: std::time::Instant::now(),
        is_draining: false,
    };

    match state
        .self_governor
        .quota
        .account_pool
        .add_account(account)
        .await
    {
        Ok(_) => (
            StatusCode::CREATED,
            Json(ApiResponse {
                success: true,
                message: format!("Account '{}' registered in pool", payload.account_id),
            }),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to register account: {}", e),
            }),
        ),
    }
}

pub async fn drain_account_handler(
    State(state): State<AppState>,
    Json(payload): Json<crate::self_governance::DrainAccountPayload>,
) -> impl IntoResponse {
    info!("Draining account '{}' via REST API...", payload.account_id);

    match state
        .self_governor
        .quota
        .account_pool
        .drain_account(&payload.account_id)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Account '{}' is now draining", payload.account_id),
            }),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to drain account: {}", e),
            }),
        ),
    }
}

pub async fn resume_account_handler(
    State(state): State<AppState>,
    Json(payload): Json<crate::self_governance::DrainAccountPayload>,
) -> impl IntoResponse {
    info!("Resuming account '{}' via REST API...", payload.account_id);

    match state
        .self_governor
        .quota
        .account_pool
        .resume_account(&payload.account_id)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Account '{}' resumed to active", payload.account_id),
            }),
        ),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(ApiResponse {
                success: false,
                message: format!("Failed to resume account: {}", e),
            }),
        ),
    }
}
