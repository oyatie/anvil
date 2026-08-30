//! Submitting a review, and the door that refuses to lose its findings.
//!
//! Two doors rather than one optional argument. A review with inline comments
//! needs the diff those comments are anchored in; a review without them does
//! not. Making the diff optional is what let the reviewer submit every finding
//! against an empty diff, drop all of them, and report success -- so the door
//! with no diff refuses a review that carries comments, and names the other.

use anyhow::{Result, bail};

use super::{GitHubClient, reviews};
use crate::reviewer::ReviewResponse;

impl GitHubClient {
    /// Submits a review that carries no inline comments.
    ///
    /// Refuses one that does. Anchoring a comment needs the diff it is anchored
    /// in, and a door that accepts comments it cannot anchor drops them --
    /// which is how the reviewer came to post zero inline comments while
    /// reporting that it had submitted a review. Use
    /// [`GitHubClient::submit_pr_review_with_diff`] for a review with findings.
    pub async fn submit_pr_review(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
        review: &ReviewResponse,
    ) -> Result<()> {
        if !review.comments.is_empty() {
            bail!(
                "{}#{}: this review carries {} inline comment(s) and no diff to anchor them in. \
                 Call `submit_pr_review_with_diff`; dropping them silently is what made the \
                 reviewer post none at all.",
                repo,
                pr_number,
                review.comments.len()
            );
        }
        reviews::submit_pr_review_impl(repo, pr_number, head_sha, review).await
    }

    /// Submits a review, keeping the inline comments `diff` proves are
    /// addressable and logging every one it drops.
    pub async fn submit_pr_review_with_diff(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
        review: &ReviewResponse,
        diff: &str,
    ) -> Result<()> {
        reviews::submit_pr_review_with_diff(repo, pr_number, head_sha, review, diff).await
    }
}
