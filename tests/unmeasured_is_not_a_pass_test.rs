//! An unmeasured gate must never be published as a passing one.
//!
//! `gate_counts()` scored `GateStatus::is_acceptable()` as "passed". That is
//! true for `NotMeasured`, so a report in which *nothing was measured* rendered
//! as the strongest claim the system can make. Merge admission was never fooled
//! -- `is_admissible()` separately requires `unmeasured_gates` to be empty --
//! but the number published to the pull request and the scorecard was.
//!
//! This is the red half of the pair: it fails against the old accounting.

use anvil::pre_merge_guard::report::PreMergeCertificationReport;

#[test]
fn a_report_that_measured_nothing_publishes_no_passes() {
    let report = PreMergeCertificationReport::unmeasured("nothing ran");
    let counts = report.gate_counts();

    assert_eq!(
        counts.passed, 0,
        "a report where every gate is NotMeasured published {} passes; \
         absent evidence is not a pass",
        counts.passed
    );
    assert_eq!(
        counts.unmeasured,
        counts.total(),
        "every gate should be counted as unmeasured, got {counts:?}"
    );
    assert_eq!(counts.failed, 0, "nothing was measured, so nothing failed");
}

#[test]
fn every_gate_is_counted_exactly_once() {
    let report = PreMergeCertificationReport::unmeasured("nothing ran");
    let counts = report.gate_counts();
    assert_eq!(
        counts.total(),
        report.all_statuses().len(),
        "the four buckets must partition the corpus, got {counts:?}"
    );
}
