//! Whether a dispatch is admitted into the review pipeline.
//!
//! Split from `review.rs`, which is over ADR-0719 D-35's budget, and separate
//! from the run for a second reason: this decision is a pure function of
//! durable state and one flag, so it can be enumerated in a test, while the
//! run it guards cannot be executed without a forge.
//!
//! Nearly every caller of `execute_pr_review` passes `force: false` -- the boot
//! recovery sweep, the `pull_request` webhook, and the manual door unless asked
//! otherwise -- so this is where a pull request is either recovered or left
//! where it is.

use crate::state::PrState;

/// What the pipeline does with a dispatch, and why.
///
/// The reason is carried rather than reconstructed at the log site. The
/// operator-facing complaint in the issue this answers is that the sweep logged
/// "Dispatched review and 70-gate certification" for pull requests it then
/// silently skipped; a decision that states itself cannot do that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// No review is stamped for this head.
    Unreviewed,
    /// The caller asked for this head to be reviewed again.
    Forced,
    /// Stamped for this head by a run that never reached the end of the
    /// pipeline. Nothing else will pick it up.
    Recovering,
    /// Reviewed for this head by a run that finished.
    Skip,
}

impl Admission {
    /// Whether the pipeline stops here.
    pub fn is_skip(self) -> bool {
        matches!(self, Self::Skip)
    }

    /// The sentence the pipeline logs for this decision.
    pub fn reason(self) -> &'static str {
        match self {
            Self::Unreviewed => "no review is recorded for this head; reviewing",
            Self::Forced => "already reviewed at this head, and the caller asked anyway; reviewing",
            Self::Recovering => {
                "reviewed at this head by a run that never finished -- a restart stranded it \
                 here, and no later commit is coming; recovering"
            }
            Self::Skip => "already reviewed at this head by a run that finished; skipping",
        }
    }
}

/// Decide whether this dispatch reviews `head_sha`.
///
/// `Recovering` is the narrow case and the reason this function exists.
/// Widening it to "already reviewed" would re-review every pull request the
/// pipeline finished and deliberately halted -- a model turn and a second
/// posted review each, on every boot, forever. Narrowing it away is the defect
/// it replaces: the recovery sweep refused exactly the pull requests it exists
/// to recover, because the stamp that strands one is also what it carries.
///
/// `prior` is read by the caller under the per-PR lock, which is what bounds
/// the concurrent case: a run already in flight for this head holds that lock
/// until after it has recorded completion, so a dispatch queued behind it
/// re-reads a finished state and skips rather than reviewing the head twice.
pub fn admit(force: bool, prior: Option<&PrState>, head_sha: &str) -> Admission {
    let Some(prior) = prior else {
        return Admission::Unreviewed;
    };
    if prior.last_reviewed_head_sha.is_empty() || prior.last_reviewed_head_sha != head_sha {
        return Admission::Unreviewed;
    }
    if force {
        return Admission::Forced;
    }
    if prior.is_stranded_at(head_sha) {
        return Admission::Recovering;
    }
    Admission::Skip
}
