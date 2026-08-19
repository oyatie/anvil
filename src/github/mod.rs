use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{info, warn};

pub mod graphql;
pub mod reviews;

use crate::reviewer::ReviewResponse;
pub use graphql::{GitHubGraphQLClient, ReviewThreadNode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMetadata {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub base_ref_name: String,
    pub base_ref_oid: String,
    pub head_ref_name: String,
    pub head_ref_oid: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubReviewComment {
    pub id: u64,
    pub path: Option<String>,
    pub line: Option<u64>,
    pub body: String,
    pub user: Option<GitHubUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubUser {
    pub login: String,
}

#[derive(Deserialize)]
struct GhPrViewOutput {
    number: u64,
    title: String,
    body: Option<String>,
    #[serde(rename = "baseRefName")]
    base_ref_name: String,
    #[serde(rename = "baseRefOid")]
    base_ref_oid: String,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    #[serde(rename = "headRefOid")]
    head_ref_oid: String,
    state: String,
}

pub struct GitHubClient;

impl Default for GitHubClient {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn check_auth(&self) -> Result<()> {
        let output = Command::new("gh")
            .args(["auth", "status"])
            .output()
            .await
            .context("Failed to execute `gh auth status`")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`gh auth status` failed: {}", stderr);
        }
        Ok(())
    }

    pub async fn ensure_webhook_extension(&self) -> Result<()> {
        let output = Command::new("gh")
            .args(["extension", "list"])
            .output()
            .await
            .context("Failed to list gh extensions")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("gh-webhook") && !stdout.contains("cli/gh-webhook") {
            info!("Installing gh-webhook extension...");
            let install_out = Command::new("gh")
                .args(["extension", "install", "cli/gh-webhook"])
                .output()
                .await
                .context("Failed to install gh-webhook")?;

            if !install_out.status.success() {
                warn!(
                    "Could not install gh-webhook: {}",
                    String::from_utf8_lossy(&install_out.stderr)
                );
            }
        }
        Ok(())
    }

    pub async fn cleanup_stale_forward_webhooks(&self, repo: &str) -> Result<()> {
        let list_out = Command::new("gh")
            .args(["api", &format!("repos/{}/hooks", repo)])
            .output()
            .await;

        if let Ok(out) = list_out {
            if out.status.success() {
                if let Ok(hooks) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) {
                    for hook in hooks {
                        if let Some(config) = hook.get("config") {
                            let url = config.get("url").and_then(|u| u.as_str()).unwrap_or("");
                            if url.contains("forwarder") || url.contains("webhook.github.com") {
                                if let Some(id) = hook.get("id").and_then(|i| i.as_u64()) {
                                    let _ = Command::new("gh")
                                        .args([
                                            "api",
                                            "--method",
                                            "DELETE",
                                            &format!("repos/{}/hooks/{}", repo, id),
                                        ])
                                        .output()
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn fetch_pr_metadata(&self, repo: &str, pr_number: u64) -> Result<PrMetadata> {
        let output = Command::new("gh")
            .args([
                "pr",
                "view",
                &pr_number.to_string(),
                "--repo",
                repo,
                "--json",
                "number,title,body,baseRefName,baseRefOid,headRefName,headRefOid,state",
            ])
            .output()
            .await
            .context("Failed to fetch PR details from GitHub CLI")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`gh pr view` failed for {}#{}: {}", repo, pr_number, stderr);
        }

        let raw: GhPrViewOutput = serde_json::from_slice(&output.stdout)?;
        Ok(PrMetadata {
            number: raw.number,
            title: raw.title,
            body: raw.body,
            base_ref_name: raw.base_ref_name,
            base_ref_oid: raw.base_ref_oid,
            head_ref_name: raw.head_ref_name,
            head_ref_oid: raw.head_ref_oid,
            state: raw.state,
        })
    }

    pub async fn submit_pr_review(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
        review: &ReviewResponse,
    ) -> Result<()> {
        reviews::submit_pr_review_impl(repo, pr_number, head_sha, review).await
    }

    /// Resolves an open review thread via GitHub GraphQL API
    pub async fn resolve_review_thread(&self, thread_id: &str) -> Result<()> {
        GitHubGraphQLClient::resolve_review_thread(thread_id).await
    }

    /// Fetches all review threads for a pull request
    pub async fn fetch_review_threads(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<ReviewThreadNode>> {
        GitHubGraphQLClient::fetch_review_threads(repo, pr_number).await
    }

    pub async fn post_pr_comment(&self, repo: &str, pr_number: u64, body: &str) -> Result<()> {
        let output = Command::new("gh")
            .args([
                "pr",
                "comment",
                &pr_number.to_string(),
                "--repo",
                repo,
                "--body",
                body,
            ])
            .output()
            .await
            .context("Failed to post comment via `gh pr comment`")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`gh pr comment` failed: {}", stderr);
        }

        info!(
            "Successfully posted review comment on {}#{}",
            repo, pr_number
        );
        Ok(())
    }

    /// Finds and updates an existing comment matching the given marker, or creates a new one
    pub async fn upsert_pr_comment(
        &self,
        repo: &str,
        pr_number: u64,
        marker: &str,
        body: &str,
    ) -> Result<()> {
        let list_endpoint = format!("repos/{}/issues/{}/comments", repo, pr_number);
        let output = Command::new("gh")
            .args(["api", &list_endpoint])
            .output()
            .await
            .context("Failed to fetch PR issue comments from GitHub API")?;

        if output.status.success() {
            #[derive(Deserialize)]
            struct IssueCommentItem {
                id: u64,
                body: Option<String>,
            }

            if let Ok(comments) = serde_json::from_slice::<Vec<IssueCommentItem>>(&output.stdout) {
                if let Some(existing) = comments
                    .iter()
                    .find(|c| c.body.as_ref().map(|b| b.contains(marker)).unwrap_or(false))
                {
                    info!(
                        "Found existing Anvil comment #{} on {}#{}. Updating in-place...",
                        existing.id, repo, pr_number
                    );
                    let patch_endpoint = format!("repos/{}/issues/comments/{}", repo, existing.id);
                    let patch_out = Command::new("gh")
                        .args([
                            "api",
                            "--method",
                            "PATCH",
                            &patch_endpoint,
                            "-f",
                            &format!("body={}", body),
                        ])
                        .output()
                        .await;

                    if let Ok(res) = patch_out {
                        if res.status.success() {
                            info!("Successfully updated comment #{} in-place", existing.id);
                            return Ok(());
                        }
                    }
                }
            }
        }

        // Fallback: Post new comment if no existing comment found or update failed
        self.post_pr_comment(repo, pr_number, body).await
    }

    pub async fn fetch_review_comments(
        &self,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<GitHubReviewComment>> {
        let endpoint = format!("repos/{}/pulls/{}/comments", repo, pr_number);
        let output = Command::new("gh")
            .args(["api", &endpoint])
            .output()
            .await
            .context("Failed to fetch review comments from GitHub API")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`gh api` failed for {}#{}: {}", repo, pr_number, stderr);
        }

        let comments: Vec<GitHubReviewComment> = serde_json::from_slice(&output.stdout)?;
        Ok(comments)
    }

    pub async fn reply_to_review_comment(
        &self,
        repo: &str,
        pr_number: u64,
        comment_id: u64,
        body: &str,
    ) -> Result<()> {
        let endpoint = format!(
            "repos/{}/pulls/{}/comments/{}/replies",
            repo, pr_number, comment_id
        );
        let output = Command::new("gh")
            .args([
                "api",
                "--method",
                "POST",
                &endpoint,
                "-f",
                &format!("body={}", body),
            ])
            .output()
            .await
            .context("Failed to reply to review comment")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`gh api` reply failed: {}", stderr);
        }

        info!(
            "Successfully replied to review comment #{} on {}#{}",
            comment_id, repo, pr_number
        );
        Ok(())
    }
}
