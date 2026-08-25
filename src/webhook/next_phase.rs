//! What anvil does next, decided from the verdict rather than left implicit.
//!
//! # Why this is not in `pipelines/`
//!
//! It was, and `pipeline_assigns_no_numeric_literal_measurement` refused it:
//! `src/webhook/pipelines/` is the code that OBTAINS measurements, and a
//! numeric literal assigned there is the absence of one.
//! `MAX_AUTO_FIX_ATTEMPTS` is a policy bound rather than a measurement, but the
//! gate is right about the boundary -- a directory whose job is to go and find
//! out should not also be where answers are written down. The decision is
//! policy; `pipelines/` executes it.
//!
//! A review that ends in a verdict and stops is a review nobody acts on. The
//! verdict is the input to a state machine:
//!
//! ```text
//!   APPROVE         + admissible  -> enlist into the merge queue
//!   REQUEST_CHANGES + actionable  -> run the fixer, then review the new head
//!   anything else                 -> halt, and say exactly why
//! ```
//!
//! Both live arms already had half an implementation: the approve arm enlisted,
//! and the reject arm did nothing at all -- `execute_pr_fix` was reachable only
//! from the CLI and one manual HTTP handler, so a pull request anvil asked to
//! change sat until a human noticed.
//!
//! # Why this is a value and not an `if` in the pipeline
//!
//! The decision has five ways to refuse and each needs a different sentence for
//! whoever is waiting. Deciding inline makes those sentences log lines that
//! nobody reads and nothing tests. Here the choice is a value: the pipeline
//! executes it, the tests assert it, and `Halt` carries the reason rather than
//! leaving a pull request stopped for no stated cause.
//!
//! # The loop this must not become
//!
//! Fixing pushes a commit, which changes the head, which triggers a review,
//! which can ask for changes again. Three bounds, and all three are refusals
//! rather than silence:
//!
//! 1. **Once per head.** A fixer run that pushes nothing leaves the head where
//!    it was, so the same head never gets a second attempt. This is what stops
//!    a fixer that cannot satisfy the review from running forever.
//! 2. **A cap per pull request.** Even when every attempt does push, the chain
//!    stops after [`MAX_AUTO_FIX_ATTEMPTS`]. A review asking for something the
//!    fixer cannot produce is a decision for a person.
//! 3. **Never a fork.** A cross-repository head cannot be pushed to, so the
//!    attempt would fail after doing the work.

use crate::state::PrState;

/// How many times anvil may rewrite one pull request before a person decides.
///
/// Three, not "until it converges". A fixer that has been asked three times and
/// still cannot satisfy the review is not going to on the fourth, and the cost
/// of finding out is a rewritten branch nobody asked for.
pub const MAX_AUTO_FIX_ATTEMPTS: u32 = 3;

/// The verdict string an approving review carries.
pub const VERDICT_APPROVE: &str = "APPROVE";
/// The verdict string a review carries when it wants the change altered.
pub const VERDICT_REQUEST_CHANGES: &str = "REQUEST_CHANGES";

/// What happens after the verdict is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextPhase {
    /// Certified and admissible: hand it to the merge queue.
    Enlist,
    /// The review asked for changes and anvil may still act on them.
    AutoFix { attempt: u32 },
    /// Nothing further happens automatically. Carries the reason, because a
    /// pull request that simply stops is indistinguishable from one anvil
    /// forgot.
    Halt { reason: String },
}

/// Everything the decision reads. Grouped so the decision stays pure and the
/// pipeline does the fetching.
pub struct Situation<'a> {
    pub verdict: &'a str,
    /// `Ok(())` when the certification report admits the pull request.
    pub admissible: Result<(), String>,
    pub head_sha: &'a str,
    /// A fork's head cannot be pushed to.
    pub is_cross_repository: bool,
    /// How many review comments the fixer would have to work from.
    pub actionable_comments: usize,
    pub state: Option<&'a PrState>,
}

/// Decide what to do next.
pub fn next_phase(s: &Situation<'_>) -> NextPhase {
    if s.verdict == VERDICT_APPROVE {
        return match &s.admissible {
            Ok(()) => NextPhase::Enlist,
            // Approved by the reviewer and refused by the corpus. The reviewer
            // does not outrank a gate, and saying so is the point: "approved"
            // with no merge and no reason is the state that makes people stop
            // trusting the queue.
            Err(why) => NextPhase::Halt {
                reason: format!(
                    "the review approved this change and the gate corpus did not admit it: {why}"
                ),
            },
        };
    }

    if s.verdict != VERDICT_REQUEST_CHANGES {
        return NextPhase::Halt {
            reason: format!(
                "the review returned `{}`, which is neither an approval nor a request for \
                 changes, so there is no next phase to run",
                s.verdict
            ),
        };
    }

    if s.is_cross_repository {
        return NextPhase::Halt {
            reason: "the head is on a fork, which anvil cannot push to; the requested changes \
                     are the contributor's to make"
                .to_string(),
        };
    }

    if s.actionable_comments == 0 {
        return NextPhase::Halt {
            reason: "the review requested changes but left no comment to act on, so there is \
                     nothing for the fixer to work from"
                .to_string(),
        };
    }

    let attempts = s.state.map(|st| st.auto_fix_attempts).unwrap_or(0);
    if attempts >= MAX_AUTO_FIX_ATTEMPTS {
        return NextPhase::Halt {
            reason: format!(
                "anvil has already rewritten this pull request {attempts} time(s); the review \
                 still asks for changes, and what it is asking for is now a decision for a person"
            ),
        };
    }

    if s.state
        .and_then(|st| st.last_auto_fixed_head_sha.as_deref())
        == Some(s.head_sha)
    {
        return NextPhase::Halt {
            reason: format!(
                "the fixer already ran against {} and pushed nothing, so running it again \
                 would produce the same result",
                &s.head_sha[..s.head_sha.len().min(8)]
            ),
        };
    }

    NextPhase::AutoFix {
        attempt: attempts + 1,
    }
}
