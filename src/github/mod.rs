use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tracing::{info, warn};

pub mod fork_guard;
pub mod reviews;

use crate::exec::{ExecClass, run_bounded};
use crate::reviewer::ReviewResponse;

/// How long a caller waits for GitHub to agree about a head it has just pushed,
/// and how often it asks. See `GitHubClient::fetch_pr_metadata_at`.
const HEAD_AGREEMENT_ATTEMPTS: u32 = 6;
const HEAD_AGREEMENT_DELAY: std::time::Duration = std::time::Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrMetadata {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub base_ref_name: String,
    pub base_ref_oid: String,
    pub head_ref_name: String,
    pub head_ref_oid: String,
    /// True when the PR head lives in a fork rather than the base repository.
    ///
    /// Without this, a fork PR is indistinguishable from a same-repo PR, and
    /// `git push origin HEAD:<head_ref_name>` targets the BASE repository's
    /// branch of that name. A fork PR with head branch "dev" or "main" would
    /// therefore push straight into the base repo, bypassing review, the gate
    /// matrix and the merge queue.
    #[serde(default)]
    pub is_cross_repository: bool,
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
    #[serde(rename = "isCrossRepository", default)]
    is_cross_repository: bool,
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
        let mut cmd = Command::new("gh");
        cmd.args(["auth", "status"]);
        let output = run_bounded(cmd, ExecClass::Api, "gh auth status")
            .await
            .context("Failed to execute `gh auth status`")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`gh auth status` failed: {}", stderr);
        }
        Ok(())
    }

    pub async fn ensure_webhook_extension(&self) -> Result<()> {
        let mut cmd = Command::new("gh");
        cmd.args(["extension", "list"]);
        let output = run_bounded(cmd, ExecClass::Api, "gh extension list")
            .await
            .context("Failed to list gh extensions")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.contains("gh-webhook") && !stdout.contains("cli/gh-webhook") {
            info!("Installing gh-webhook extension...");
            let mut install_cmd = Command::new("gh");
            install_cmd.args(["extension", "install", "cli/gh-webhook"]);
            let install_out = run_bounded(
                install_cmd,
                ExecClass::Vcs,
                "gh extension install cli/gh-webhook",
            )
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
        let mut list_cmd = Command::new("gh");
        list_cmd.args(["api", &format!("repos/{}/hooks", repo)]);
        let list_out = run_bounded(list_cmd, ExecClass::Api, "gh api repos/:repo/hooks").await;

        if let Ok(out) = list_out
            && out.status.success()
            && let Ok(hooks) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout)
        {
            for hook in hooks {
                if let Some(config) = hook.get("config") {
                    let url = config.get("url").and_then(|u| u.as_str()).unwrap_or("");
                    if (url.contains("forwarder") || url.contains("webhook.github.com"))
                        && let Some(id) = hook.get("id").and_then(|i| i.as_u64())
                    {
                        let mut del_cmd = Command::new("gh");
                        del_cmd.args([
                            "api",
                            "--method",
                            "DELETE",
                            &format!("repos/{}/hooks/{}", repo, id),
                        ]);
                        let _ = run_bounded(
                            del_cmd,
                            ExecClass::Api,
                            "gh api DELETE stale forwarder webhook",
                        )
                        .await;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn fetch_pr_metadata(&self, repo: &str, pr_number: u64) -> Result<PrMetadata> {
        let mut cmd = Command::new("gh");
        cmd.args([
            "pr",
            "view",
            &pr_number.to_string(),
            "--repo",
            repo,
            "--json",
            "number,title,body,baseRefName,baseRefOid,headRefName,headRefOid,state,isCrossRepository,headRepositoryOwner",
        ]);
        let output = run_bounded(cmd, ExecClass::Api, "gh pr view")
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
            is_cross_repository: raw.is_cross_repository,
            state: raw.state,
        })
    }

    /// The pull request's metadata, once GitHub reports `expected_head` as its
    /// head.
    ///
    /// GitHub's view of a pull request head is eventually consistent
    /// immediately after a push: `git push` returns, and a `gh pr view`
    /// microseconds later can still name the pre-push commit. Any caller that
    /// just pushed and then asks which commit the pull request is on -- the
    /// queue healer does exactly this -- is racing that window, and a
    /// single-shot comparison turns an ordinary, named race into a refusal.
    ///
    /// So the read is retried on a bounded backoff, and the mismatch is fatal
    /// only once GitHub has had `HEAD_AGREEMENT_ATTEMPTS` chances to catch up.
    /// Still fail-closed at the end: certifying whichever commit the API happens
    /// to name would produce evidence about a commit nobody asked about.
    ///
    /// With `expected_head` `None` the caller holds no belief about the head and
    /// the first read is the answer.
    pub async fn fetch_pr_metadata_at(
        &self,
        repo: &str,
        pr_number: u64,
        expected_head: Option<&str>,
    ) -> Result<PrMetadata> {
        let mut seen = String::new();
        for attempt in 1..=HEAD_AGREEMENT_ATTEMPTS {
            let meta = self.fetch_pr_metadata(repo, pr_number).await?;
            let Some(expected) = expected_head else {
                return Ok(meta);
            };
            if meta.head_ref_oid == expected {
                return Ok(meta);
            }
            seen = meta.head_ref_oid;
            if attempt < HEAD_AGREEMENT_ATTEMPTS {
                info!(
                    "GitHub still reports {} as the head of {}#{} and {} was expected; waiting {}s \
                     for its view to catch up (attempt {} of {}).",
                    seen,
                    repo,
                    pr_number,
                    expected,
                    HEAD_AGREEMENT_DELAY.as_secs(),
                    attempt,
                    HEAD_AGREEMENT_ATTEMPTS
                );
                tokio::time::sleep(HEAD_AGREEMENT_DELAY).await;
            }
        }
        bail!(
            "the head expected for {}#{} is {}, and GitHub still reports {} after {} attempts. \
             Acting on the commit the API happens to name would act on a commit nobody asked \
             about.",
            repo,
            pr_number,
            expected_head.unwrap_or_default(),
            seen,
            HEAD_AGREEMENT_ATTEMPTS
        )
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

    pub async fn post_pr_comment(&self, repo: &str, pr_number: u64, body: &str) -> Result<()> {
        let mut cmd = Command::new("gh");
        cmd.args([
            "pr",
            "comment",
            &pr_number.to_string(),
            "--repo",
            repo,
            "--body",
            body,
        ]);
        let output = run_bounded(cmd, ExecClass::Api, "gh pr comment")
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
        let mut cmd = Command::new("gh");
        cmd.args(["api", &list_endpoint]);
        let output = run_bounded(cmd, ExecClass::Api, "gh api list PR issue comments")
            .await
            .context("Failed to fetch PR issue comments from GitHub API")?;

        if output.status.success() {
            #[derive(Deserialize)]
            struct IssueCommentItem {
                id: u64,
                body: Option<String>,
            }

            if let Ok(comments) = serde_json::from_slice::<Vec<IssueCommentItem>>(&output.stdout)
                && let Some(existing) = comments
                    .iter()
                    .find(|c| c.body.as_ref().map(|b| b.contains(marker)).unwrap_or(false))
            {
                info!(
                    "Found existing Anvil comment #{} on {}#{}. Updating in-place...",
                    existing.id, repo, pr_number
                );
                let patch_endpoint = format!("repos/{}/issues/comments/{}", repo, existing.id);
                let mut patch_cmd = Command::new("gh");
                patch_cmd.args([
                    "api",
                    "--method",
                    "PATCH",
                    &patch_endpoint,
                    "-f",
                    &format!("body={}", body),
                ]);
                let patch_out =
                    run_bounded(patch_cmd, ExecClass::Api, "gh api PATCH issue comment").await;

                if let Ok(res) = patch_out
                    && res.status.success()
                {
                    info!("Successfully updated comment #{} in-place", existing.id);
                    return Ok(());
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
        let mut cmd = Command::new("gh");
        cmd.args(["api", &endpoint]);
        let output = run_bounded(cmd, ExecClass::Api, "gh api list PR review comments")
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
        let mut cmd = Command::new("gh");
        cmd.args([
            "api",
            "--method",
            "POST",
            &endpoint,
            "-f",
            &format!("body={}", body),
        ]);
        let output = run_bounded(cmd, ExecClass::Api, "gh api POST review comment reply")
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

    /// Fetches all open pull requests for a given repository
    pub async fn list_open_prs(&self, repo: &str) -> Result<Vec<PrMetadata>> {
        let mut cmd = Command::new("gh");
        cmd.args([
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            "open",
            "--json",
            "number,title,body,baseRefName,baseRefOid,headRefName,headRefOid,state,isCrossRepository,headRepositoryOwner",
        ]);
        let output = run_bounded(cmd, ExecClass::Api, "gh pr list --state open")
            .await
            .context("Failed to list open PRs from GitHub CLI")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`gh pr list` failed for {}: {}", repo, stderr);
        }

        let raw: Vec<GhPrViewOutput> = serde_json::from_slice(&output.stdout)?;
        let prs = raw
            .into_iter()
            .map(|r| PrMetadata {
                number: r.number,
                title: r.title,
                body: r.body,
                base_ref_name: r.base_ref_name,
                base_ref_oid: r.base_ref_oid,
                head_ref_name: r.head_ref_name,
                head_ref_oid: r.head_ref_oid,
                is_cross_repository: r.is_cross_repository,
                state: r.state,
            })
            .collect();
        Ok(prs)
    }

    /// Fetches the latest commit SHA for a specific branch
    pub async fn fetch_branch_sha(&self, repo: &str, branch: &str) -> Result<String> {
        let endpoint = format!("repos/{}/commits/{}", repo, branch);
        let mut cmd = Command::new("gh");
        cmd.args(["api", &endpoint, "--jq", ".sha"]);
        let output = run_bounded(cmd, ExecClass::Api, "gh api branch commit sha")
            .await
            .context("Failed to fetch branch commit SHA from GitHub API")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("`gh api` failed for {}/{}: {}", repo, branch, stderr);
        }

        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(sha)
    }

    /// Fetches merge queue depth for a branch
    pub async fn fetch_merge_queue_depth(&self, repo: &str, _branch: &str) -> Result<usize> {
        // Query PRs in merge_queue or currently running checks in merge group
        let mut cmd = Command::new("gh");
        cmd.args([
            "pr",
            "list",
            "--repo",
            repo,
            "--search",
            "status:in-progress",
            "--json",
            "number",
        ]);
        let output = run_bounded(cmd, ExecClass::Api, "gh pr list merge queue depth")
            .await
            .context("Failed to fetch merge queue depth")?;

        if !output.status.success() {
            return Ok(0);
        }

        #[derive(Deserialize)]
        struct SimplePr {
            #[allow(dead_code)]
            number: u64,
        }

        let prs: Vec<SimplePr> = serde_json::from_slice(&output.stdout).unwrap_or_default();
        Ok(prs.len())
    }

    /// Computes live empirical DORA metrics directly from GitHub PR and Actions workflow histories
    pub async fn fetch_repo_dora_metrics(
        &self,
        repo: &str,
    ) -> Result<crate::telemetry_store::DoraMetricSnapshot> {
        #[derive(Deserialize)]
        struct MergedPrInfo {
            #[serde(rename = "createdAt")]
            created_at: String,
            #[serde(rename = "mergedAt")]
            merged_at: Option<String>,
        }

        // 1. Fetch merged PRs
        let mut pr_cmd = Command::new("gh");
        pr_cmd.args([
            "pr",
            "list",
            "--repo",
            repo,
            "--state",
            "merged",
            "--limit",
            "20",
            "--json",
            "number,createdAt,mergedAt",
        ]);
        let pr_output = run_bounded(pr_cmd, ExecClass::Api, "gh pr list --state merged (DORA)")
            .await
            .context("Failed to fetch merged PRs for DORA calculation")?;

        let merged_prs: Vec<MergedPrInfo> = if pr_output.status.success() {
            serde_json::from_slice(&pr_output.stdout).unwrap_or_default()
        } else {
            Vec::new()
        };

        let mut total_lead_hours = 0.0;
        let mut valid_lead_count = 0;

        for pr in &merged_prs {
            if let Some(merged_str) = &pr.merged_at
                && let (Ok(created), Ok(merged)) = (
                    chrono::DateTime::parse_from_rfc3339(&pr.created_at),
                    chrono::DateTime::parse_from_rfc3339(merged_str),
                )
            {
                let diff_mins = (merged - created).num_minutes() as f64;
                if diff_mins > 0.0 {
                    total_lead_hours += diff_mins / 60.0;
                    valid_lead_count += 1;
                }
            }
        }

        let lead_time_hours = if valid_lead_count > 0 {
            total_lead_hours / valid_lead_count as f64
        } else {
            0.0
        };

        let total_deployments_30d = merged_prs.len();
        let deployment_frequency_per_day = total_deployments_30d as f64 / 30.0;

        // 2. Fetch Workflow Runs for Change Failure Rate and MTTR
        #[derive(Deserialize)]
        struct RunInfo {
            conclusion: Option<String>,
            #[serde(rename = "createdAt")]
            created_at: String,
            #[serde(rename = "updatedAt")]
            updated_at: String,
        }

        let mut run_cmd = Command::new("gh");
        run_cmd.args([
            "run",
            "list",
            "--repo",
            repo,
            "--limit",
            "20",
            "--json",
            "conclusion,createdAt,updatedAt",
        ]);
        let run_output = run_bounded(run_cmd, ExecClass::Api, "gh run list (DORA)")
            .await
            .context("Failed to fetch workflow runs for CFR calculation")?;

        let runs: Vec<RunInfo> = if run_output.status.success() {
            serde_json::from_slice(&run_output.stdout).unwrap_or_default()
        } else {
            Vec::new()
        };

        let completed_runs: Vec<_> = runs
            .into_iter()
            .filter(|r| !r.conclusion.as_deref().unwrap_or("").is_empty())
            .collect();

        let total_runs = completed_runs.len();
        let failed_runs = completed_runs
            .iter()
            .filter(|r| r.conclusion.as_deref() == Some("failure"))
            .count();

        let change_failure_rate_percent = if total_runs > 0 {
            (failed_runs as f64 / total_runs as f64) * 100.0
        } else {
            0.0
        };

        let mut mttr_minutes = 0.0;
        let mut incident_count = 0;

        for r in &completed_runs {
            if r.conclusion.as_deref() == Some("failure")
                && let (Ok(created), Ok(updated)) = (
                    chrono::DateTime::parse_from_rfc3339(&r.created_at),
                    chrono::DateTime::parse_from_rfc3339(&r.updated_at),
                )
            {
                let duration = (updated - created).num_minutes() as f64;
                mttr_minutes += duration.max(2.0);
                incident_count += 1;
            }
        }

        let avg_mttr = if incident_count > 0 {
            mttr_minutes / incident_count as f64
        } else {
            0.0
        };

        Ok(crate::telemetry_store::DoraMetricSnapshot {
            repo: repo.to_string(),
            timestamp: chrono::Utc::now(),
            lead_time_for_changes_hours: lead_time_hours,
            deployment_frequency_per_day,
            change_failure_rate_percent,
            mean_time_to_restore_mins: avg_mttr,
            total_deployments_30d,
            total_incidents_30d: incident_count,
        })
    }
}
