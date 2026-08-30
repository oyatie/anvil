//! The shape of a GitHub webhook delivery, as Anvil reads it.
//!
//! Only the fields a door acts on are declared. Serde ignores the rest, so a
//! payload GitHub extends still parses.

use serde::Deserialize;

use crate::github::identity::Actor;

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

/// Who GitHub says acted.
///
/// `id` and `user_type` are the typed identity the payload already carries:
/// a stable numeric actor id, and one of "User", "Bot" or "Organization".
/// Both are optional so that a delivery omitting either still parses --
/// dropping the whole comment over a missing field loses the comment too --
/// and `github::identity::answerable_by` refuses on an absent type rather
/// than reading it as "not a bot".
#[derive(Deserialize, Debug)]
pub struct WebhookUser {
    pub login: String,
    pub id: Option<u64>,
    #[serde(rename = "type")]
    pub user_type: Option<String>,
}

impl WebhookUser {
    /// The identity the loop-guard decides on. See `github::identity`.
    pub fn actor(&self) -> Actor {
        Actor {
            login: self.login.clone(),
            id: self.id,
            kind: self.user_type.clone(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct WebhookRepository {
    pub full_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The typed fields arrive from the wire, under GitHub's own names.
    #[test]
    fn a_user_carries_its_id_and_type_off_the_wire() {
        let user: WebhookUser =
            serde_json::from_str(r#"{"login":"abbott","id":1234,"type":"User"}"#)
                .expect("a user payload parses");
        assert_eq!(user.actor().id, Some(1234));
        assert_eq!(user.actor().kind.as_deref(), Some("User"));
    }

    /// A payload without them still parses; the fields read as unknown.
    #[test]
    fn a_user_without_them_still_parses() {
        let user: WebhookUser =
            serde_json::from_str(r#"{"login":"abbott"}"#).expect("a bare login parses");
        assert_eq!(user.actor().id, None);
        assert_eq!(user.actor().kind, None);
    }
}
