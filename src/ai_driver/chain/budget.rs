//! What a caller's bound leaves for the next tier.
//!
//! A budget handed to [`super::run_stage_within`] is a bound on the WHOLE
//! stage, not an allowance each tier gets to spend in full. One number cannot
//! mean both, and reading it as the second breaks both callers in opposite
//! directions:
//!
//! * `queue_healer` passes `AGY_TURN_LIMIT` (600s) and its comment claimed "a
//!   chain declaring a longer timeout cannot outlive the healer's own bound."
//!   Measured against `config/model-routing.toml`, remediation declares five
//!   tiers at 300/420/420/600/600; capping each at 600 sums to 2340s. The
//!   healer believed 600 and could run 39 minutes.
//! * `doc_guard` passes its watchdog's 120s from INSIDE that watchdog. Five
//!   spec_review tiers at 480/420/600/420/600, each capped at 120, is 600s of
//!   intent under a supervisor that kills at 120 -- so tier 1 consumed the
//!   whole probe and tiers 2..5 were unreachable.
//!
//! One number cannot mean both "per attempt" and "for the stage". It means the
//! stage, and this type does the arithmetic.
//!
//! It never reads the clock. The caller measures elapsed time and hands it in,
//! which is what lets a test drive a five-tier chain to exhaustion without
//! spawning anything.

use std::time::Duration;

/// The least a tier can be allotted and still be worth spawning.
///
/// `agy_print_timeout_arg` subtracts [`crate::exec::AGY_PRINT_TIMEOUT_MARGIN`]
/// (30s) from whatever it is given and clamps the result to at least 1s, so a
/// tier handed 30s or less tells the CLI it has one second. That is not a short
/// turn, it is a turn that cannot happen -- and I1 says a tier that cannot run
/// must be reported as not tried, never as a model that answered and had
/// nothing to say. Twice the margin leaves a real turn on the far side of it.
pub const MIN_TIER_ALLOTMENT: Duration = Duration::from_secs(60);

/// A stage-wide bound, spent down as tiers run.
///
/// `None` means the caller supplied no budget: every tier gets exactly what the
/// routing table declares, which is the pre-existing behaviour for callers that
/// are not under a supervisor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageBudget {
    remaining: Option<Duration>,
}

impl StageBudget {
    /// A budget for the whole stage, or an unbounded one when `budget` is
    /// `None`.
    #[must_use]
    pub fn of(budget: Option<Duration>) -> Self {
        Self { remaining: budget }
    }

    /// What is left for the stage, or `None` when unbounded.
    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        self.remaining
    }

    /// The bound for a tier declaring `declared`: the lesser of what the tier
    /// asks for and what the stage has left.
    ///
    /// `None` means STOP, not skip. What remains cannot produce a turn, so
    /// every tier from here on is untried -- and the caller must say so rather
    /// than spawn something it knows will be cut off.
    #[must_use]
    pub fn allot(&self, declared: Duration) -> Option<Duration> {
        match self.remaining {
            None => Some(declared),
            Some(left) if left >= MIN_TIER_ALLOTMENT => Some(left.min(declared)),
            Some(_) => None,
        }
    }

    /// Deduct what a tier actually took.
    ///
    /// Actual, not declared: a provider that refuses in two seconds must not
    /// cost the chain the ten minutes it was entitled to, or one fast refusal
    /// would strand every tier behind it.
    pub fn spend(&mut self, elapsed: Duration) {
        if let Some(left) = self.remaining.as_mut() {
            *left = left.saturating_sub(elapsed);
        }
    }
}
