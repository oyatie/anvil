//! Publication evidence comes from the forge response, not the request intent.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use tracing::{info, warn};

#[derive(Debug)]
pub struct RecordedReview {
    id: u64,
    commit_id: String,
    state: String,
}

#[derive(Deserialize)]
struct ResponseReview {
    id: u64,
    commit_id: String,
    state: String,
}

impl RecordedReview {
    pub(super) fn from_response(bytes: &[u8], head: &str, event: &str) -> Result<Self> {
        let expected_state = match event {
            "APPROVE" => "APPROVED",
            "COMMENT" => "COMMENTED",
            "REQUEST_CHANGES" => "CHANGES_REQUESTED",
            _ => bail!("Unsupported requested review event"),
        };
        let receipt: ResponseReview = serde_json::from_slice(bytes)
            .context("Formal review POST did not return a valid review receipt")?;
        if receipt.id == 0
            || head.is_empty()
            || receipt.commit_id != head
            || receipt.state != expected_state
        {
            bail!("Formal review POST receipt does not establish the requested state and head");
        }
        Ok(Self {
            id: receipt.id,
            commit_id: receipt.commit_id,
            state: receipt.state,
        })
    }

    pub fn require_approved_for(&self, head: &str) -> Result<()> {
        if self.id == 0 || head.is_empty() || self.commit_id != head || self.state != "APPROVED" {
            bail!("Merge admission requires a recorded APPROVED review for the exact head");
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum ReviewPublication {
    RecordedReview(RecordedReview),
    SummaryCommentOnly,
}

impl ReviewPublication {
    pub(super) fn require_recorded(self) -> Result<RecordedReview> {
        match self {
            Self::RecordedReview(receipt) => Ok(receipt),
            Self::SummaryCommentOnly => bail!(
                "Review findings were published as a summary comment, but no formal review was recorded"
            ),
        }
    }

    pub fn report(&self, repo: &str, pr_number: u64) {
        match self {
            Self::RecordedReview(receipt) => info!(
                "{repo}#{pr_number}: recorded formal review {} in state {} for {}",
                receipt.id, receipt.state, receipt.commit_id
            ),
            Self::SummaryCommentOnly => warn!(
                "{repo}#{pr_number}: findings published as summary comment only; no formal review recorded"
            ),
        }
    }
}

#[cfg(test)]
mod tests;
