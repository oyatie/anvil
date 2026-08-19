use anyhow::Result;
use tracing::info;

use crate::fixer::ReviewFeedbackItem;
use crate::webhook::AppState;

pub async fn execute_pr_fix(state: &AppState, repo: &str, pr_number: u64) -> Result<()> {
    info!("Running Auto-Fixer for PR #{} on {}...", pr_number, repo);
    let meta = state
        .github_client
        .fetch_pr_metadata(repo, pr_number)
        .await?;
    let comments = state
        .github_client
        .fetch_review_comments(repo, pr_number)
        .await?;

    let feedback_items: Vec<ReviewFeedbackItem> = comments
        .into_iter()
        .map(|c| ReviewFeedbackItem {
            comment_id: Some(c.id),
            file_path: c.path,
            line: c.line,
            body: c.body,
            author: c
                .user
                .map(|u| u.login)
                .unwrap_or_else(|| "reviewer".to_string()),
        })
        .collect();

    state
        .fixer
        .resolve_and_fix(
            repo,
            pr_number,
            &meta.head_ref_name,
            &meta.head_ref_oid,
            &feedback_items,
        )
        .await?;

    Ok(())
}
