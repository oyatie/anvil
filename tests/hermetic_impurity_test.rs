//! The hermetic gate has two halves; only one of them is unmeasured.

use anvil::hermetic_build::HermeticBuildValidator;
use anvil::pre_merge_guard::GateStatus;

/// The digest comparison is unmeasured -- nothing builds this tree twice.
/// The impurity scan is not: it fires on code someone would plausibly write,
/// and it is why the registry grades this gate `Heuristic`.
///
/// An earlier revision of this PR replaced the whole gate with `NotMeasured`
/// and threw the working half away.
#[test]
fn an_impure_diff_fails_even_though_no_binary_was_compared() {
    let report = HermeticBuildValidator::new()
        .scan_for_impurity_without_build_pair("+const T: &str = SystemTime::now();");

    assert!(
        matches!(report.status, GateStatus::Failed(_)),
        "a build embedding wall-clock time cannot reproduce; got {:?}",
        report.status
    );
    assert!(!report.passed);
}

/// And a clean diff is still not a reproducible build, because nothing
/// compared two binaries. Without this the gate could satisfy the test above
/// by failing everything.
#[test]
fn a_clean_diff_is_unmeasured_rather_than_reproducible() {
    let report =
        HermeticBuildValidator::new().scan_for_impurity_without_build_pair("+let n = 1 + 1;");

    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some("hermetic_build_status"),
        "no impurity found is not the same as two binaries matching"
    );
    assert!(!report.passed);
}
