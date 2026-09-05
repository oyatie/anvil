//! Whether a pull-request event is work anvil owns.
//!
//! # Why this is a function and not three `if`s in the handler
//!
//! It was three `if`s in the handler, and the decision was therefore
//! unreachable: `webhook_handler` takes HTTP state and returns a response, so
//! "should this pull request be reviewed?" could only be exercised by standing
//! up a server. A decision nothing can call is a decision nothing can measure,
//! and the gap that produced this module is what that costs.
//!
//! `ready_for_review` was missing from the supported actions. GitHub sends it
//! the moment a draft becomes ready — the exact moment the work becomes
//! anvil's — and nothing matched it, so a finished pull request sat untouched
//! until some later push happened to emit `synchronize`. Meanwhile `opened` and
//! `synchronize` fire regardless of draft state, so drafts *were* reviewed. The
//! behaviour was inverted: review the unfinished, ignore the finished.
//!
//! Neither half is visible in a test that goes through HTTP, and neither was
//! caught by 65 passing tests over this file.

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
/// `ready_for_review` is in this list and `opened`/`synchronize` are gated by
/// the draft check below, which together are the whole fix: adding the action
/// without the draft gate reviews drafts *and* ready PRs; adding the gate
/// without the action ignores both.
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
        // The defect this module exists for. GitHub clears `draft` before it
        // sends `ready_for_review`, so the payload arrives ready.
        assert_eq!(
            admit("ready_for_review", false, "a title"),
            PrAdmission::Review
        );
    }

    #[test]
    fn a_draft_is_not_reviewed_on_any_action_that_carries_one() {
        // The other half. `opened` and `synchronize` fire while still a draft,
        // and used to be reviewed because nothing looked at the flag.
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
