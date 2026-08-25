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

// ---------------------------------------------------------------------------
// What the gate actually runs
// ---------------------------------------------------------------------------
//
// The status mapping above was correct and the gate underneath it was not.
// `local_verification_gate` (certify.rs) creates an ephemeral worktree at the
// pull request head, proves the tree is that commit with
// `EphemeralWorktree::verify_at`, and then calls
// `QueueHealer::run_local_test_gate` -- which, for a Cargo repository, ran
// `cargo check`. That type-checks; it builds no test binary and executes no
// test. So the gate named "Automated Test Suite", whose failure sentence is
// "Test suite reported failures during verification gate" and whose
// remediation is "fix the failing tests locally before pushing", certified on
// every Rust pull request that the tests pass having run none of them. A tree
// in which every test fails passed it.
//
// These tests are behavioural, not textual: each builds a real crate on disk
// and runs the real gate against it. A source scan asserting the word "test"
// appears in the command would be satisfied by `cargo test --no-run`, which is
// the same defect with a longer name.
//
// They deliberately do NOT invoke Anvil's own suite -- a test-running gate
// under test recurses -- and each fixture crate has zero dependencies, so it
// builds offline in about a second.

use anvil::queue_healer::{QueueHealer, TestGate};

/// A minimal, dependency-free crate whose `src/lib.rs` is `lib_rs`.
fn crate_with(lib_rs: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"gate-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [dependencies]\n",
    )
    .expect("write Cargo.toml");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir src");
    std::fs::write(dir.path().join("src/lib.rs"), lib_rs).expect("write lib.rs");
    dir
}

const FAILING_SUITE: &str = "pub fn two() -> u32 { 3 }\n\
    #[cfg(test)]\nmod tests {\n    #[test]\n    fn two_is_two() { assert_eq!(super::two(), 2); }\n}\n";

const PASSING_SUITE: &str = "pub fn two() -> u32 { 2 }\n\
    #[cfg(test)]\nmod tests {\n    #[test]\n    fn two_is_two() { assert_eq!(super::two(), 2); }\n}\n";

/// The property the gate is named for, and the one it did not have.
///
/// This crate compiles cleanly and its single test fails. `cargo check` is
/// silent about that, so before this change the gate returned
/// `TestGate::Passed("cargo check")` and the corpus published
/// `test_suite_status: Passed` for a suite that was red.
#[tokio::test]
async fn a_tree_whose_tests_fail_produces_a_failing_gate() {
    let dir = crate_with(FAILING_SUITE);

    let gate = QueueHealer::run_local_test_gate(dir.path()).await;

    assert!(
        matches!(gate, TestGate::Failed(_)),
        "a tree whose tests fail must fail the test-suite gate; got {gate:?}. \
         A gate that compiles the code and reports a pass certifies a red suite \
         as green on every Rust pull request."
    );
}

/// The other half, so the fix cannot be "always fail".
///
/// Without this, replacing the gate body with `TestGate::Failed(label)` passes
/// the test above and accuses every pull request in the fleet.
#[tokio::test]
async fn a_tree_whose_tests_pass_produces_a_passing_gate() {
    let dir = crate_with(PASSING_SUITE);

    let gate = QueueHealer::run_local_test_gate(dir.path()).await;

    assert!(
        matches!(gate, TestGate::Passed(_)),
        "a tree whose tests pass must pass the gate; got {gate:?}"
    );
}

/// A tree that does not build measured nothing, and must not be accused of a
/// failing suite.
///
/// `cargo`'s documented exit statuses are `0` and `101` only, and libtest also
/// exits `101` when tests fail, so one `cargo test` invocation cannot tell a
/// compile error from a test failure. The gate therefore builds as its own
/// step. Before this change the same collapse happened one level up: `cargo
/// check` failing on an unbuildable tree became `TestGate::Failed`, which
/// `local_verification_gate` mapped to `Some(false)` and the scorecard
/// published as "Test suite reported failures during verification gate", with a
/// remediation to fix tests that were never built.
#[tokio::test]
async fn a_tree_that_does_not_build_is_not_measured_rather_than_accused() {
    let dir = crate_with("pub fn broken( -> u32 { 2 }\n");

    let gate = QueueHealer::run_local_test_gate(dir.path()).await;

    match gate {
        TestGate::Errored(_, cause) => assert!(
            !cause.trim().is_empty(),
            "`the gate did not complete` is only actionable with the reason"
        ),
        other => panic!(
            "an unbuildable tree ran no test, so it is absent evidence and not a \
             failing suite; got {other:?}"
        ),
    }
}

/// A repository offering no gate Anvil knows how to run is `Unavailable`, which
/// `local_verification_gate` maps to `None` and the corpus to `NotMeasured`.
/// Pinned so the Cargo/npm selection cannot start defaulting to a verdict.
#[tokio::test]
async fn a_repository_with_no_gate_offers_no_verdict() {
    let dir = tempfile::tempdir().expect("tempdir");

    assert_eq!(
        QueueHealer::run_local_test_gate(dir.path()).await,
        TestGate::Unavailable,
        "no Cargo.toml and no npm test script is no gate, not a pass"
    );
}
