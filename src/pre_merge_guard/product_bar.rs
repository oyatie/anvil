//! The Product seat's measurement: the bet and the acceptance bar.
//!
//! ADR-0002, Discover §1. Job: the bet and the acceptance bar. Artifact: a
//! written problem plus a done-when. Measurement: quality sign-off cannot sign
//! off without it.
//!
//! The artifact is authored on the change under review, so this gate reads the
//! change's own title and body — the same metadata `doc_guard` and `reviewer`
//! already receive — and makes no network or filesystem call.
//!
//! Absence is the defect itself. A change that never wrote a bar has not
//! produced evidence this gate could not read; it produced no bar. That is
//! `Failed`, not `NotMeasured` and not `Warning`.
//!
//! NOT IMPLEMENTED YET. The body below is `todo!()` on purpose: the suite in
//! `tests/product_seat_done_when_test.rs` is the specification and was written
//! first.
//!
//! Nor is it wired. `evaluate_pre_merge_gates` neither receives the change's
//! title and body nor calls this function; the report carries a placeholder
//! instead. That is deliberate. The wiring is part of the specification — a
//! flawless `judge` reached from nothing gates nothing — so it is pinned by
//! three tests at the bottom of that suite, and those tests have to be red
//! before the wiring exists, like every other test here.

use super::GateStatus;

/// Judges the Product artifact carried by the change under review.
#[allow(unused_variables)]
pub fn judge(pr_title: &str, pr_body: &str) -> GateStatus {
    todo!("Product seat measurement; the specification is tests/product_seat_done_when_test.rs")
}
