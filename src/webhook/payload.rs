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
    /// Only an explicit false establishes that the pull request is not a draft.
    /// Missing or null status stays unknown, so shared event shapes still parse
    /// without authorizing lifecycle review.
    pub draft: Option<bool>,
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
    use crate::webhook::pr_admission::{PrAdmission, SkipReason, admit};

    fn lifecycle_payload(action: &str, draft: Option<serde_json::Value>) -> serde_json::Value {
        let mut value = serde_json::json!({
            "action": action,
            "repository": {"full_name": "example/project"},
            "pull_request": {
                "number": 1,
                "title": "Update documentation",
                "head": {"sha": "head", "ref": "documentation"},
                "base": {"sha": "base", "ref": "dev"}
            }
        });
        if let Some(draft) = draft {
            value["pull_request"]["draft"] = draft;
        }
        value
    }

    fn parsed_admission(value: serde_json::Value) -> PrAdmission {
        let payload: GitHubWebhookPayload =
            serde_json::from_value(value).expect("ordinary lifecycle payload parses");
        let pr = payload.pull_request.expect("fixture has a pull request");
        admit(payload.action.as_deref().unwrap_or(""), pr.draft, &pr.title)
    }

    #[test]
    fn explicit_draft_status_controls_every_reviewable_action() {
        for action in ["opened", "synchronize", "reopened", "ready_for_review"] {
            for (draft, expected) in [
                (false, PrAdmission::Review),
                (true, PrAdmission::Skip(SkipReason::Draft)),
            ] {
                assert_eq!(
                    parsed_admission(lifecycle_payload(action, Some(draft.into()))),
                    expected,
                    "action {action}, draft {draft}"
                );
            }
        }
    }

    #[test]
    fn missing_draft_status_never_authorizes_lifecycle_review() {
        for action in ["opened", "synchronize", "reopened", "ready_for_review"] {
            assert_eq!(
                parsed_admission(lifecycle_payload(action, None)),
                PrAdmission::Skip(SkipReason::UnknownDraftStatus),
                "missing draft status authorized {action}"
            );
        }
    }

    #[test]
    fn null_draft_status_parses_but_never_authorizes_lifecycle_review() {
        for action in ["opened", "synchronize", "reopened", "ready_for_review"] {
            assert_eq!(
                parsed_admission(lifecycle_payload(action, Some(serde_json::Value::Null))),
                PrAdmission::Skip(SkipReason::UnknownDraftStatus),
                "null draft status authorized {action}"
            );
        }
    }

    #[test]
    fn wrong_draft_types_are_parse_errors() {
        for draft in [
            serde_json::json!("false"),
            serde_json::json!(0),
            serde_json::json!({}),
            serde_json::json!([]),
        ] {
            assert!(
                serde_json::from_value::<GitHubWebhookPayload>(lifecycle_payload(
                    "opened",
                    Some(draft)
                ))
                .is_err()
            );
        }
    }

    #[test]
    fn parsed_unsupported_actions_remain_refused() {
        for action in ["closed", "converted_to_draft", ""] {
            assert_eq!(
                parsed_admission(lifecycle_payload(action, Some(false.into()))),
                PrAdmission::Skip(SkipReason::UnsupportedAction)
            );
        }
    }

    #[test]
    fn shared_event_shapes_do_not_require_draft_evidence_to_parse() {
        let comment_shape = lifecycle_payload("created", None);
        let parsed: GitHubWebhookPayload =
            serde_json::from_value(comment_shape).expect("shared PR shape parses");
        assert_eq!(parsed.pull_request.expect("shared PR shape").draft, None);
        let without_pr = serde_json::json!({
            "action": "completed",
            "repository": {"full_name": "example/project"},
            "workflow_run": {"id": 1}
        });
        let parsed: GitHubWebhookPayload =
            serde_json::from_value(without_pr).expect("non-PR shape parses");
        assert!(parsed.pull_request.is_none());
    }

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
