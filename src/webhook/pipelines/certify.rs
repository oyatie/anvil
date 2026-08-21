use anyhow::Result;
use tracing::info;

use super::review::execute_pr_review;
use crate::webhook::AppState;

pub async fn execute_pr_certify(state: &AppState, repo: &str, pr_number: u64) -> Result<()> {
    info!(
        "Running Pre-Merge Certification for PR #{} on {}...",
        pr_number, repo
    );
    let meta = state
        .github_client
        .fetch_pr_metadata(repo, pr_number)
        .await?;
    execute_pr_review(
        state,
        repo,
        pr_number,
        &meta.title,
        &meta.body.unwrap_or_default(),
        &meta.base_ref_name,
        &meta.base_ref_oid,
        &meta.head_ref_oid,
        true,
    )
    .await
}
