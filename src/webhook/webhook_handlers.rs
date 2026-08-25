use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use subtle::ConstantTimeEq;
use tracing::{error, info, warn};

use super::pipelines::execute_pr_review;
use super::{ApiResponse, AppState};
use crate::fixer::ReviewFeedbackItem;
use crate::queue_healer::QueueHealer;

/// Verifies GitHub X-Hub-Signature-256 HMAC in constant time to prevent timing attacks
pub fn verify_github_hmac(secret: &str, raw_bytes: &[u8], signature_header: Option<&str>) -> bool {
    let signature = match signature_header {
        Some(sig) => sig,
        None => return false,
    };

    let expected_hex = match signature.strip_prefix("sha256=") {
        Some(hex) => hex,
        None => signature,
    };

    let expected_bytes = match hex::decode(expected_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };

    let mut mac = match Hmac::<Sha256>::new_from_slice(secret.as_bytes()) {
        Ok(m) => m,
        Err(_) => return false,
    };

    mac.update(raw_bytes);
    let result = mac.finalize().into_bytes();

    result.as_slice().ct_eq(&expected_bytes).into()
}

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
    /// Present on pull_request payloads. Comparing head.repo to base.repo is the
    /// payload-side equivalent of `isCrossRepository`: it identifies a fork PR,
    /// whose head branch name must never be used as a push target against the
    /// base repository. See github::fork_guard.
    #[serde(default)]
    pub repo: Option<WebhookRepository>,
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
    body_bytes: Bytes,
) -> impl IntoResponse {
    // 1. Constant-time HMAC-SHA256 verification (Zero-Trust Ingress Security).
    //
    // ENFORCING. This ran in observe mode until the relay question was settled
    // empirically: it was not established that the original X-Hub-Signature-256
    // survives `gh webhook forward`'s relay through webhook-forwarder.github.com,
    // and enforcing on an assumption would have produced a daemon that boots,
    // reports healthy, and silently rejects every delivery.
    //
    // Evidence for promotion, measured on live traffic rather than assumed:
    // `gh api repos/{r}/hooks` showed secret_set flip false -> true on all three
    // watched repositories once `--secret` was passed to the forwarder, and six
    // consecutive real deliveries logged signature_present=true
    // signature_valid=true. The signature does survive the relay.
    //
    // A delivery signed with GITHUB_WEBHOOK_SECRET_PREVIOUS is still accepted so
    // a secret rotation does not drop in-flight deliveries; see config.rs.
    match &state.config.webhook_secret {
        Some(secret) => {
            let sig = headers
                .get("x-hub-signature-256")
                .and_then(|v| v.to_str().ok());
            let matched_primary = verify_github_hmac(secret, &body_bytes, sig);
            let matched_previous = !matched_primary
                && state
                    .config
                    .webhook_secret_previous
                    .as_deref()
                    .is_some_and(|prev| verify_github_hmac(prev, &body_bytes, sig));

            if matched_previous {
                warn!(
                    "[Webhook Ingress] delivery verified against GITHUB_WEBHOOK_SECRET_PREVIOUS; \
                     rotation is in progress. Clear the previous secret once deliveries stop matching it."
                );
            }

            if !(matched_primary || matched_previous) {
                warn!(
                    "🚨 [Webhook Ingress] rejecting delivery: signature_present={} signature_valid=false",
                    sig.is_some()
                );
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(ApiResponse {
                        success: false,
                        message: "Invalid or missing X-Hub-Signature-256 signature".to_string(),
                    }),
                );
            }
        }
        None => {
            // Unauthenticated ingress drives clone -> AI -> commit -> push with
            // attacker-chosen fields. Refusing is the only safe response; the
            // daemon also refuses to boot without the secret (config.rs).
            warn!(
                "🚨 [Webhook Ingress] rejecting delivery: GITHUB_WEBHOOK_SECRET is not configured, \
                 so no delivery can be authenticated."
            );
            return (
                StatusCode::UNAUTHORIZED,
                Json(ApiResponse {
                    success: false,
                    message: "Webhook signature verification is not configured".to_string(),
                }),
            );
        }
    }

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

    let payload: GitHubWebhookPayload = match serde_json::from_slice(&body_bytes) {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse {
                    success: false,
                    message: format!("Invalid JSON payload: {}", e),
                }),
            );
        }
    };

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
                        message: format!(
                            "Skipped review for automated PR {}#{}",
                            repo_name, pr_number
                        ),
                    }),
                );
            }

            // Anti-Loop Filter 2: Check if already certified and in merge queue
            if let Some(prior) = state.state_mgr.get_pr_state(&repo_name, pr_number).await
                && prior.last_certified_head_sha.as_deref() == Some(&head_sha)
                && prior.is_enlisted_in_merge_queue
            {
                info!(
                    "PR {}#{} head {} is already 100% certified and in merge queue. Dropping webhook loop.",
                    repo_name, pr_number, head_sha
                );
                return (
                    StatusCode::OK,
                    Json(ApiResponse {
                        success: true,
                        message: format!(
                            "PR {}#{} is already certified and queued",
                            repo_name, pr_number
                        ),
                    }),
                );
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
                    error!(
                        "Failed to execute PR review for {}#{}: {:?}",
                        repo_clone, pr_number, e
                    );
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
    if event_type == "pull_request_review_comment"
        && action == "created"
        && let (Some(pr), Some(comment)) = (&payload.pull_request, &payload.comment)
    {
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
        // Fork detection from the payload: differing head/base repositories
        // is the equivalent of `isCrossRepository`. Unknown (either side
        // absent) is treated as cross-repo -- absent evidence must not
        // authorise a push (invariant I1). See github::fork_guard.
        let is_cross_repository = match (
            pr.head.repo.as_ref().map(|r| r.full_name.as_str()),
            pr.base.repo.as_ref().map(|r| r.full_name.as_str()),
        ) {
            (Some(h), Some(b)) => !h.eq_ignore_ascii_case(b),
            _ => true,
        };

        tokio::spawn(async move {
            let _ = state_clone
                .fixer
                .resolve_and_fix(
                    &repo_clone,
                    pr_number,
                    &head_branch,
                    &head_sha,
                    is_cross_repository,
                    &[feedback_item],
                )
                .await;
        });

        return (
            StatusCode::ACCEPTED,
            Json(ApiResponse {
                success: true,
                message: format!(
                    "Resolution queued for comment on {}#{}",
                    repo_name, pr.number
                ),
            }),
        );
    }

    // Case 3: Trunk Workflow Run Failure on main/dev (workflow_run)
    if event_type == "workflow_run"
        && action == "completed"
        && let Some(wf) = payload.workflow_run
    {
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
                        .triage_workflow_run(
                            &repo_clone,
                            run_id,
                            &branch_str,
                            &commit_sha,
                            &wf_name,
                            &repo_dir,
                        )
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

    // Case 4: Merge Group Events (merge_group)
    // Only trigger auto-healing when a merge group is destroyed due to check failure
    if event_type == "merge_group"
        && action == "destroyed"
        && let Some(mg) = payload.merge_group
        && let Some(pr_number) = QueueHealer::extract_pr_number_from_merge_ref(&mg.head_ref)
    {
        let state_clone = state.clone();
        let repo_clone = repo_name.clone();

        // This door is a webhook delivery, so there is nobody to answer with
        // the outcome and the heal is genuinely detached. The outcome is still
        // consumed: `heal_in_worktree` now returns the refusal that used to be
        // a `warn!` inside it, and dropped into `let _` in a detached task it
        // would be logged nowhere at all -- the one enlist door where the
        // refusal would have become *less* observable than before.
        tokio::spawn(async move {
            match state_clone
                .queue_healer
                .heal_ejected_pr(&state_clone, &repo_clone, pr_number)
                .await
            {
                Ok(what_happened) => info!("{}", what_happened),
                Err(e) => warn!(
                    "Automatic queue heal for {}#{} did not complete: {:#}",
                    repo_clone, pr_number, e
                ),
            }
        });

        return (
            StatusCode::ACCEPTED,
            Json(ApiResponse {
                success: true,
                message: format!(
                    "Queue healing monitored for PR #{} on {}",
                    pr_number, repo_name
                ),
            }),
        );
    }

    (
        StatusCode::OK,
        Json(ApiResponse {
            success: true,
            message: format!("Ignored event: {}/{}", event_type, action),
        }),
    )
}

#[cfg(test)]
mod hmac_tests {
    use super::*;
    use hmac::Mac;

    fn sign(secret: &str, body: &[u8]) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn accepts_a_correctly_signed_body() {
        let body = br#"{"action":"opened"}"#;
        assert!(verify_github_hmac(
            "s3cr3t",
            body,
            Some(&sign("s3cr3t", body))
        ));
    }

    #[test]
    fn rejects_wrong_secret_missing_header_and_tampered_body() {
        let body = br#"{"action":"opened"}"#;
        let sig = sign("s3cr3t", body);
        assert!(!verify_github_hmac("other", body, Some(&sig)));
        assert!(!verify_github_hmac("s3cr3t", body, None));
        assert!(!verify_github_hmac(
            "s3cr3t",
            br#"{"action":"closed"}"#,
            Some(&sig)
        ));
        assert!(!verify_github_hmac("s3cr3t", body, Some("sha256=zzzz")));
        assert!(!verify_github_hmac("s3cr3t", body, Some("")));
    }

    /// The rotation window: a delivery signed with the OLD secret must still
    /// verify while GITHUB_WEBHOOK_SECRET_PREVIOUS is set, and must stop
    /// verifying once it is cleared. This is what makes rotation lossless.
    #[test]
    fn rotation_window_accepts_old_signatures_then_stops() {
        let body = br#"{"action":"synchronize"}"#;
        let old_sig = sign("old-secret", body);

        // New secret alone does not accept an old-signed delivery.
        assert!(!verify_github_hmac("new-secret", body, Some(&old_sig)));
        // The previous secret does -- this is the fallback the handler consults.
        assert!(verify_github_hmac("old-secret", body, Some(&old_sig)));
        // After the window closes, the old signature is refused.
        assert!(!verify_github_hmac("unrelated", body, Some(&old_sig)));
    }
}
