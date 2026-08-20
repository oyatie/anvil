use anyhow::{bail, Context, Result};
use serde::Serialize;
use tokio::process::Command;
use tracing::{info, warn};

use crate::exec::{run_bounded, ExecClass};
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
        .map(|c| {
            let body_with_footer = if c.body.contains("🤖 Reviewed by Oyatie Anvil") {
                c.body.clone()
            } else {
                format!(
                    "{}\n\n---\n*🤖 Reviewed by Oyatie Anvil*",
                    c.body.trim_end()
                )
            };
            ReviewCommentPayload {
                path: c.path.clone(),
                line: c.line,
                body: body_with_footer,
            }
        })
        .collect();

    let summary_with_footer = if review.summary.contains("🤖 Reviewed by Oyatie Anvil") {
        review.summary.clone()
    } else {
        format!(
            "{}\n\n---\n*🤖 Reviewed by Oyatie Anvil*",
            review.summary.trim_end()
        )
    };

    let request = CreateReviewRequest {
        commit_id: head_sha.to_string(),
        body: summary_with_footer,
        event: review.verdict.clone(),
        comments: comments_payload,
    };

    let json_body = serde_json::to_string(&request)?;
    let endpoint = format!("repos/{}/pulls/{}/reviews", repo, pr_number);

    // `gh api --input -` needs a piped stdin, but the bounded runner drives the
    // child through `Command::output()`, which closes stdin. Handing `gh` the
    // same JSON through a temp file sends a byte-identical request body while
    // keeping the call under a timeout with `kill_on_drop`.
    let mut body_file = tempfile::NamedTempFile::new()
        .context("Failed to create a temp file for the gh api request body")?;
    {
        use std::io::Write;
        body_file
            .write_all(json_body.as_bytes())
            .context("Failed to write the gh api request body")?;
        body_file
            .flush()
            .context("Failed to flush the gh api request body")?;
    }
    let body_path = body_file.path().to_string_lossy().into_owned();

    let mut cmd = Command::new("gh");
    cmd.args(["api", "--method", "POST", &endpoint, "--input", &body_path]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let output = run_bounded(cmd, ExecClass::Api, "gh api POST pull request review").await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        warn!(
            "Submitting full review with inline comments failed (lines might not be in diff): {}. Retrying with summary and fallback comments.",
            stderr
        );

        return submit_fallback_review(repo, pr_number, review).await;
    }

    info!(
        "Successfully published PR review for {}#{}",
        repo, pr_number
    );
    Ok(())
}

async fn submit_fallback_review(repo: &str, pr_number: u64, review: &ReviewResponse) -> Result<()> {
    let mut full_body = review.summary.clone();

    if !review.comments.is_empty() {
        full_body.push_str("\n\n### 📝 Detailed Inline Findings:\n");
        for c in &review.comments {
            full_body.push_str(&format!("- **`{}:{}`**\n  {}\n", c.path, c.line, c.body));
        }
    }

    let endpoint = format!("repos/{}/issues/{}/comments", repo, pr_number);
    let mut cmd = Command::new("gh");
    cmd.args([
        "api",
        "--method",
        "POST",
        &endpoint,
        "-f",
        &format!("body={}", full_body),
    ]);

    let output = run_bounded(cmd, ExecClass::Api, "gh api POST fallback review comment").await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Fallback review comment failed: {}", stderr);
    }

    info!(
        "Successfully submitted PR review fallback for {}#{}",
        repo, pr_number
    );
    Ok(())
}
