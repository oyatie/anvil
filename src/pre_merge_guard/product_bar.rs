//! The Product seat's measurement: the bet and the acceptance bar.
//!
//! ADR-0002, Discover §1. Job: the bet and the acceptance bar. Artifact: a
//! written problem plus a done-when. Measurement: quality sign-off cannot sign
//! off without it.
//!
//! The artifact is authored on the change under review, so this gate reads the
//! change's own body — the same metadata `doc_guard` and `reviewer` already
//! receive — and makes no network or filesystem call. That last claim is not
//! decoration: `the_verdict_depends_on_nothing_but_the_change_it_was_handed`
//! pins it, because a gate that loaded its vocabulary from disk would be a
//! flake this suite could not attribute.
//!
//! The body and nothing else. An earlier revision of these signatures also took
//! the pull request title, and no test in the suite could tell a gate that read
//! it from one that ignored it — a named input with no measurement, inside a
//! gate whose whole subject is named gates with no measurement. The suite
//! already decides the question behaviourally: a change whose bet appears only
//! in a descriptive title is still missing its written problem
//! (`the_bet_and_the_bar_are_written_on_the_change_not_left_to_its_title`), so
//! the title cannot supply either artifact and is not an input. Listed in
//! open_questions as a decision a human can veto.
//!
//! Absence is the defect itself. A change that never wrote a bar has not
//! produced evidence this gate could not read; it produced no bar. That is
//! `Failed`, not `NotMeasured` and not `Warning` — and that holds for a body
//! that uses no heading this gate recognises just as much as for an empty one.
//!
//! Two entry points, deliberately: `missing_artifacts` is the measurement and
//! `judge` is the verdict rendered from it. The split exists so the tests can
//! assert *which* artifact the gate found missing without pattern-matching on
//! the prose of the message, which would forbid the gate from quoting the
//! offending section back at the author.
//!
//! NOT IMPLEMENTED YET. Both bodies below are `todo!()` on purpose: the suite
//! in `tests/product_seat_done_when_test.rs` is the specification and was
//! written first.
//!
//! Nor is it wired. `evaluate_pre_merge_gates` neither receives the change's
//! body nor calls this function; the report carries a placeholder instead.
//! That is deliberate. The wiring is part of the specification — a
//! flawless `judge` reached from nothing gates nothing — so it is pinned by
//! three tests at the bottom of that suite, and those tests have to be red
//! before the wiring exists, like every other test here.

use super::GateStatus;

/// One half of the Product seat's artifact.
///
/// Ordered so a caller can compare sets without depending on render order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Artifact {
    /// The bet: what is wrong and why it matters.
    WrittenProblem,
    /// The acceptance bar: how anyone other than the author checks it is done.
    DoneWhenBar,
}

/// The Product artifacts this change did not produce, in canonical order.
///
/// Empty means the change carries both. This is the measurement; `judge`
/// renders its verdict and its message from it.
#[allow(unused_variables)]
pub fn missing_artifacts(pr_body: &str) -> Vec<Artifact> {
    todo!("Product seat measurement; the specification is tests/product_seat_done_when_test.rs")
}

/// Judges the Product artifact carried by the change under review.
#[allow(unused_variables)]
pub fn judge(pr_body: &str) -> GateStatus {
    todo!("Product seat verdict; the specification is tests/product_seat_done_when_test.rs")
}
