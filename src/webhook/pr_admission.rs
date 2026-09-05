//! Whether a pull-request event is work anvil owns.
//!
//! The decision is a pure function so it can be exercised without standing up
//! a server. `webhook_handler` takes HTTP state and returns a response; a
//! decision embedded there is reachable only through a live socket, and so is
//! measured by nothing.
//!
//! Two rules, and they are load-bearing only together:
//!
//! - `ready_for_review` is a reviewable action. GitHub sends it when a draft
//!   becomes ready, which is the moment the work becomes anvil's.
//! - A draft is never reviewed, on any action. `opened` and `synchronize` fire
//!   while a pull request is still a draft.
//!
//! These do not conflict: GitHub clears `draft` before sending
//! `ready_for_review`, so refusing drafts does not refuse the transition out of
//! one. Admitting the action without the draft rule reviews drafts as well as
//! ready work; the draft rule without the action ignores both.

/// What anvil should do with a `pull_request` event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrAdmission {
    /// Run the review pipeline against this head.
    Review,
    /// Not ours, with the reason a human reads in the response body.
    Skip(SkipReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// An action anvil does not act on (`closed`, `labeled`, `assigned`, ...).
    UnsupportedAction,
    /// Still a draft. `ready_for_review` clears the flag before the event is
    /// sent, so refusing drafts does not refuse the transition out of one.
    Draft,
    /// anvil's own governance sync, marked so the loop terminates.
    AutomatedPr,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            SkipReason::UnsupportedAction => "unsupported action",
            SkipReason::Draft => "draft",
            SkipReason::AutomatedPr => "automated PR",
        }
    }
}

/// The actions that carry a head anvil should review.
///
/// Anything absent here is skipped, including `closed` and `converted_to_draft`.
const REVIEWABLE_ACTIONS: &[&str] = &["opened", "synchronize", "reopened", "ready_for_review"];

/// Pure, so the decision can be exercised without a server.
pub fn admit(action: &str, draft: bool, title: &str) -> PrAdmission {
    if !REVIEWABLE_ACTIONS.contains(&action) {
        return PrAdmission::Skip(SkipReason::UnsupportedAction);
    }
    if draft {
        return PrAdmission::Skip(SkipReason::Draft);
    }
    if title.contains("[skip review]") {
        return PrAdmission::Skip(SkipReason::AutomatedPr);
    }
    PrAdmission::Review
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_draft_becoming_ready_is_reviewed() {
        // GitHub clears `draft` before it sends `ready_for_review`, so the
        // payload arrives ready and the draft rule does not catch it.
        assert_eq!(
            admit("ready_for_review", false, "a title"),
            PrAdmission::Review
        );
    }

    #[test]
    fn a_draft_is_not_reviewed_on_any_action_that_carries_one() {
        // These actions fire while a pull request is still a draft, so the
        // draft rule -- not the action list -- is what refuses them.
        for action in ["opened", "synchronize", "reopened"] {
            assert_eq!(
                admit(action, true, "a title"),
                PrAdmission::Skip(SkipReason::Draft),
                "a draft was admitted on `{action}`"
            );
        }
    }

    #[test]
    fn the_ordinary_path_still_works() {
        for action in ["opened", "synchronize", "reopened"] {
            assert_eq!(admit(action, false, "a title"), PrAdmission::Review);
        }
    }

    #[test]
    fn actions_anvil_does_not_own_are_refused() {
        for action in ["closed", "labeled", "assigned", "converted_to_draft", ""] {
            assert_eq!(
                admit(action, false, "a title"),
                PrAdmission::Skip(SkipReason::UnsupportedAction),
                "`{action}` was admitted"
            );
        }
    }

    #[test]
    fn the_governance_sync_still_terminates_its_own_loop() {
        assert_eq!(
            admit("opened", false, "chore: sync [skip review]"),
            PrAdmission::Skip(SkipReason::AutomatedPr)
        );
    }

    #[test]
    fn draft_is_checked_before_the_title() {
        // Ordering is observable, so it is pinned: a draft whose title also
        // carries the marker reports as a draft, not as automation.
        assert_eq!(
            admit("opened", true, "chore: sync [skip review]"),
            PrAdmission::Skip(SkipReason::Draft)
        );
    }
}
