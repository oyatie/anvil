//! Two decision gates demonstrate both halves.
//!
//! Neither scans a diff: `test_suite_status` is a function of one `Option<bool>`
//! and `review_verdict_status` of one verdict string. That makes their third
//! answer the interesting one — both can report NEITHER pass nor fail, and both
//! must, because "no suite ran" and "the reviewer did not finish" are absences
//! rather than verdicts. A gate that collapses those into a pass is the defect
//! `Absence` exists to name; one that collapses them into a failure accuses on
//! evidence nobody gathered.

use anvil::pre_merge_guard::evaluator::{PreMergeGuard, review_verdict_gate};
use anvil::pre_merge_guard::report::GateStatus;

// ---------------------------------------------------------------------------
// test_suite_status
// ---------------------------------------------------------------------------

#[test]
fn test_suite_fires_when_the_suite_reported_failures() {
    assert!(
        matches!(
            PreMergeGuard::test_suite_gate_status(Some(false)),
            GateStatus::Failed(_)
        ),
        "the suite reported failures and the gate did not"
    );
}

#[test]
fn test_suite_spares_a_suite_that_passed() {
    assert!(
        matches!(
            PreMergeGuard::test_suite_gate_status(Some(true)),
            GateStatus::Passed
        ),
        "a passing suite must not withhold the merge"
    );
}

/// The third answer, which is the one that matters: no suite ran.
#[test]
fn test_suite_withholds_when_nothing_was_measured() {
    assert!(
        matches!(
            PreMergeGuard::test_suite_gate_status(None),
            GateStatus::NotMeasured { .. }
        ),
        "no suite was executed, so the gate has neither a pass nor a failure to \
         report — publishing either is a claim about evidence nobody gathered"
    );
}

// ---------------------------------------------------------------------------
// review_verdict_status
// ---------------------------------------------------------------------------

#[test]
fn review_verdict_fires_on_a_blocking_verdict() {
    assert!(
        matches!(
            review_verdict_gate("REQUEST_CHANGES"),
            GateStatus::Failed(_)
        ),
        "the reviewer asked for changes and the gate did not withhold the merge"
    );
}

#[test]
fn review_verdict_spares_an_approval() {
    assert!(matches!(review_verdict_gate("APPROVE"), GateStatus::Passed));
    assert!(
        matches!(review_verdict_gate("COMMENT"), GateStatus::Passed),
        "a comment is not a refusal; treating it as one refuses every review \
         that had something to say and nothing to block"
    );
}

/// A review that did not complete is `Errored`, never `Failed`.
#[test]
fn review_verdict_does_not_report_a_refusal_the_reviewer_never_made() {
    assert!(
        matches!(
            review_verdict_gate(anvil::reviewer::VERDICT_ERRORED),
            GateStatus::Errored(_)
        ),
        "the review produced no parseable verdict, so it judged nothing. \
         Reporting `Failed` would publish a refusal the reviewer never made, \
         which is I1's symmetric violation."
    );
}
