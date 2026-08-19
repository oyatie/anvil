use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::{error, info};

use super::{ApiResponse, AppState};
use super::pipelines::execute_pr_review;
use crate::fixer::ReviewFeedbackItem;
use crate::queue_healer::QueueHealer;

#[derive(Deserialize, Debug)]
pub struct GitHubWebhookPayload {
    pub action: Option<String>,
    pub number: Option<u64>,
    pub pull_request: Option<WebhookPullRequest>,
    pub repository: Option<WebhookRepository>,
    pub comment: Option<WebhookComment>,
    pub review: Option<WebhookReview>,
    pub workflow_run: Option<WebhookWorkflowRun>,
    pub merge_group: Option<WebhookMergeGroup>,
}

#[derive(Deserialize, Debug)]
pub struct WebhookPullRequest {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub head: WebhookCommitRef,
    pub base: WebhookCommitRef,
}

#[derive(Deserialize, Debug)]
pub struct WebhookCommitRef {
    pub sha: String,
    #[serde(rename = "ref")]
    pub branch_ref: String,
}

#[derive(Deserialize, Debug)]
pub struct WebhookComment {
    pub id: u64,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub body: String,
    pub user: Option<WebhookUser>,
}

#[derive(Deserialize, Debug)]
pub struct WebhookReview {
    pub id: u64,
    pub body: Option<String>,
    pub state: Option<String>,
    pub user: Option<WebhookUser>,
}

#[derive(Deserialize, Debug)]
pub struct WebhookWorkflowRun {
    pub id: u64,
    pub name: Option<String>,
    pub head_branch: Option<String>,
    pub head_sha: Option<String>,
    pub conclusion: Option<String>,
    pub status: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct WebhookMergeGroup {
    pub head_ref: String,
    pub head_sha: String,
    pub base_ref: String,
    pub base_sha: String,
}

#[derive(Deserialize, Debug)]
pub struct WebhookUser {
    pub login: String,
}

#[derive(Deserialize, Debug)]
pub struct WebhookRepository {
    pub full_name: String,
}

pub async fn webhook_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<GitHubWebhookPayload>,
) -> impl IntoResponse {
    let event_type = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    info!("Received GitHub webhook event: {}", event_type);

    if event_type == "ping" {
        return (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: "Webhook ping received successfully".to_string(),
            }),
        );
    }

    let repo_name = payload
        .repository
        .as_ref()
        .map(|r| r.full_name.clone())
        .unwrap_or_default();

    if repo_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ApiResponse {
                success: false,
                message: "Missing repository in payload".to_string(),
            }),
        );
    }

    let is_watched = state
        .config
        .watched_repos
        .iter()
        .any(|w| w.eq_ignore_ascii_case(&repo_name));

    if !is_watched {
        return (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                message: format!("Repository {} is not watched", repo_name),
            }),
        );
    }

    let action = payload.action.as_deref().unwrap_or("");

    // Case 1: Pull Request lifecycle events (opened, synchronize, reopened)
    if event_type == "pull_request" {
        let supported_actions = ["opened", "synchronize", "reopened"];
        if !supported_actions.contains(&action) {
            return (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    message: format!("Ignored PR action: {}", action),
                }),
            );
        }

        if let Some(pr) = payload.pull_request {
            let pr_number = pr.number;
            let head_sha = pr.head.sha.clone();

            // Anti-Loop Filter 1: Ignore commits created by the automated governance sync
            if pr.title.contains("[skip review]") {
                return (
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        message: format!("Skipped review for automated PR {}#{}", repo_name, pr_number),
                    }),
                );
            }

            // Anti-Loop Filter 2: Check if already certified and in merge queue
            if let Some(prior) = state.state_mgr.get_pr_state(&repo_name, pr_number).await {
                if prior.last_certified_head_sha.as_deref() == Some(&head_sha) && prior.is_enlisted_in_merge_queue {
                    info!("PR {}#{} head {} is already 100% certified and in merge queue. Dropping webhook loop.", repo_name, pr_number, head_sha);
                    return (
                        StatusCode::OK,
                        Json(ApiResponse {
                            success: true,
                            message: format!("PR {}#{} is already certified and queued", repo_name, pr_number),
                        }),
                    );
                }
            }

            let state_clone = state.clone();
            let repo_clone = repo_name.clone();
            let pr_title = pr.title.clone();
            let pr_body = pr.body.unwrap_or_default();
            let base_branch = pr.base.branch_ref.clone();
            let base_sha = pr.base.sha.clone();

            tokio::spawn(async move {
                if let Err(e) = execute_pr_review(
                    &state_clone,
                    &repo_clone,
                    pr_number,
                    &pr_title,
                    &pr_body,
                    &base_branch,
                    &base_sha,
                    &head_sha,
                    false,
                )
                .await
                {
                    error!("Failed to execute PR review for {}#{}: {:?}", repo_clone, pr_number, e);
                }
            });

            return (
                StatusCode::ACCEPTED,
                Json(ApiResponse {
                    success: true,
                    message: format!("Review queued for {}#{}", repo_name, pr.number),
                }),
            );
        }
    }

    // Case 2: Inline Review Comment Created (pull_request_review_comment)
    if event_type == "pull_request_review_comment" && action == "created" {
        if let (Some(pr), Some(comment)) = (&payload.pull_request, &payload.comment) {
            let author = comment
                .user
                .as_ref()
                .map(|u| u.login.clone())
                .unwrap_or_else(|| "reviewer".to_string());

            if author.contains("bot") || author.contains("antigravity") {
                return (
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        message: "Ignored comment from bot".to_string(),
                    }),
                );
            }

            let feedback_item = ReviewFeedbackItem {
                comment_id: Some(comment.id),
                file_path: comment.path.clone(),
                line: comment.line,
                body: comment.body.clone(),
                author,
            };

            let state_clone = state.clone();
            let repo_clone = repo_name.clone();
            let pr_number = pr.number;
            let head_branch = pr.head.branch_ref.clone();
            let head_sha = pr.head.sha.clone();

            tokio::spawn(async move {
                let _ = state_clone
                    .fixer
                    .resolve_and_fix(&repo_clone, pr_number, &head_branch, &head_sha, &[feedback_item])
                    .await;
            });

            return (
                StatusCode::ACCEPTED,
                Json(ApiResponse {
                    success: true,
                    message: format!("Resolution queued for comment on {}#{}", repo_name, pr.number),
                }),
            );
        }
    }

    // Case 3: Trunk Workflow Run Failure on main/dev (workflow_run)
    if event_type == "workflow_run" && action == "completed" {
        if let Some(wf) = payload.workflow_run {
            let conclusion = wf.conclusion.as_deref().unwrap_or("");
            let branch = wf.head_branch.as_deref().unwrap_or("");

            if conclusion == "failure" && (branch == "main" || branch == "dev" || branch == "master") {
                let state_clone = state.clone();
                let repo_clone = repo_name.clone();
                let run_id = wf.id;
                let branch_str = branch.to_string();
                let commit_sha = wf.head_sha.unwrap_or_default();
                let wf_name = wf.name.unwrap_or_else(|| "CI Workflow".to_string());

                tokio::spawn(async move {
                    if let Ok(repo_dir) = state_clone.git_mgr.ensure_repo_cloned(&repo_clone).await {
                        let _ = state_clone
                            .ci_triager
                            .triage_workflow_run(&repo_clone, run_id, &branch_str, &commit_sha, &wf_name, &repo_dir)
                            .await;
                    }
                });

                return (
                    StatusCode::ACCEPTED,
                    Json(ApiResponse {
                        success: true,
                        message: format!("Trunk CI triage queued for run #{} on {}", wf.id, repo_name),
                    }),
                );
            }
        }
    }

    // Case 4: Merge Group Events (merge_group)
    if event_type == "merge_group" && (action == "checks_requested" || action == "destroyed") {
        if let Some(mg) = payload.merge_group {
            if let Some(pr_number) = QueueHealer::extract_pr_number_from_merge_ref(&mg.head_ref) {
                let state_clone = state.clone();
                let repo_clone = repo_name.clone();

                tokio::spawn(async move {
                    let _ = state_clone.queue_healer.heal_ejected_pr(&repo_clone, pr_number).await;
                });

                return (
                    StatusCode::ACCEPTED,
                    Json(ApiResponse {
                        success: true,
                        message: format!("Queue healing monitored for PR #{} on {}", pr_number, repo_name),
                    }),
                );
            }
        }
    }

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: format!("Ignored event: {}/{}", event_type, action),
        }),
    )
}
