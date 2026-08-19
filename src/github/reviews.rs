use anyhow::{bail, Context, Result};
use serde::Serialize;
use tokio::process::Command;
use tracing::{info, warn};

use crate::reviewer::ReviewResponse;

#[derive(Serialize)]
struct CreateReviewRequest {
    commit_id: String,
    body: String,
    event: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    comments: Vec<ReviewCommentPayload>,
}

#[derive(Serialize)]
struct ReviewCommentPayload {
    path: String,
    line: u64,
    body: String,
}

pub async fn submit_pr_review_impl(
    repo: &str,
    pr_number: u64,
    head_sha: &str,
    review: &ReviewResponse,
) -> Result<()> {
    info!(
        "Submitting PR review for {}#{} with verdict: {} ({} inline comments)",
        repo,
        pr_number,
        review.verdict,
        review.comments.len()
    );

    let comments_payload: Vec<ReviewCommentPayload> = review
        .comments
        .iter()
        .map(|c| ReviewCommentPayload {
            path: c.path.clone(),
            line: c.line,
            body: c.body.clone(),
        })
        .collect();

    let request = CreateReviewRequest {
        commit_id: head_sha.to_string(),
        body: review.summary.clone(),
        event: review.verdict.clone(),
        comments: comments_payload,
    };

    let json_body = serde_json::to_string(&request)?;
    let endpoint = format!("repos/{}/pulls/{}/reviews", repo, pr_number);

    let mut cmd = Command::new("gh");
    cmd.args(["api", "--method", "POST", &endpoint, "--input", "-"]);
    cmd.stdin(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().context("Failed to spawn gh api command")?;
    if let Some(mut stdin) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        let _ = stdin.write_all(json_body.as_bytes()).await;
    }

    let output = child.wait_with_output().await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "Submitting full review with inline comments failed (lines might not be in diff): {}. Retrying with summary and fallback comments.",
            stderr
        );

        return submit_fallback_review(repo, pr_number, review).await;
    }

    info!("Successfully published PR review for {}#{}", repo, pr_number);
    Ok(())
}

async fn submit_fallback_review(
    repo: &str,
    pr_number: u64,
    review: &ReviewResponse,
) -> Result<()> {
    let mut full_body = review.summary.clone();

    if !review.comments.is_empty() {
        full_body.push_str("\n\n### 📝 Detailed Inline Findings:\n");
        for c in &review.comments {
            full_body.push_str(&format!(
                "- **`{}:{}`**\n  {}\n",
                c.path, c.line, c.body
            ));
        }
    }

    let endpoint = format!("repos/{}/issues/{}/comments", repo, pr_number);
    let mut cmd = Command::new("gh");
    cmd.args(["api", "--method", "POST", &endpoint, "-f", &format!("body={}", full_body)]);

    let output = cmd.output().await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Fallback review comment failed: {}", stderr);
    }

    info!("Successfully submitted PR review fallback for {}#{}", repo, pr_number);
    Ok(())
}
