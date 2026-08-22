//! The test-suite gate must not certify a suite that never ran.
//!
//! Lives in `tests/` rather than beside the code: `evaluator_gate_ordering_test`
//! scans `evaluator.rs` for gate-status bindings that appear after the report
//! literal, and an in-file test module trips that guard. The guard is right --
//! a status computed after the literal is invisible to the verdict -- so the
//! test moved rather than the rule bending.

use anvil::pre_merge_guard::{GateStatus, PreMergeGuard};

/// `test_suite_passed` was a `bool` and the review pipeline passed the
/// literal `true` (review.rs:608). So `test_suite_status` -- the gate
/// labelled "Automated Test Suite" -- certified on every pull request that
/// the tests pass, while nothing in the pipeline runs a test.
///
/// Of every gate in this fabric, this is the one whose name most directly
/// asserts a thing that was never done.
#[test]
fn an_unrun_suite_is_not_measured_rather_than_passed() {
    let unmeasured = PreMergeGuard::test_suite_gate_status(None);
    assert_eq!(
        unmeasured.unmeasured_gate_id(),
        Some("test_suite_status"),
        "a suite nobody ran must not report a verdict"
    );

    assert!(matches!(
        PreMergeGuard::test_suite_gate_status(Some(true)),
        GateStatus::Passed
    ));
    assert!(matches!(
        PreMergeGuard::test_suite_gate_status(Some(false)),
        GateStatus::Failed(_)
    ));
}
