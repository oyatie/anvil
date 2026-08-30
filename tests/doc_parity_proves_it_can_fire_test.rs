//! The doc-parity gate demonstrates both halves, and the ordering between them.
//!
//! Four outcomes, and the ORDER they are consulted in is the defect this gate
//! carried: `AutoUpdated.is_acceptable()` is `true`, so consulting the written
//! file list before the adverse finding let a stub the guard wrote for an
//! under-documented diff certify the very gap it is evidence of. Work done is
//! not evidence about the diff.

use anvil::doc_guard::DocGuardReport;
use anvil::pre_merge_guard::evaluator::doc_parity_status;
use anvil::pre_merge_guard::report::GateStatus;

fn report(sufficient: bool, wrote: &[&str], errored: Option<&str>) -> DocGuardReport {
    DocGuardReport {
        is_sufficient: sufficient,
        files_created_or_updated: wrote.iter().map(|s| s.to_string()).collect(),
        summary: "docs".to_string(),
        errored: errored.map(str::to_string),
    }
}

#[test]
fn doc_parity_fires_on_an_under_documented_change() {
    assert!(
        matches!(
            doc_parity_status(&report(false, &[], None)),
            GateStatus::Failed(_)
        ),
        "the guard judged the documentation insufficient and the gate did not \
         withhold the merge"
    );
}

#[test]
fn doc_parity_spares_a_change_whose_documentation_is_sufficient() {
    assert!(
        matches!(
            doc_parity_status(&report(true, &[], None)),
            GateStatus::Passed
        ),
        "sufficient documentation must not withhold the merge"
    );
}

/// The ordering. A stub written FOR an under-documented diff must not certify it.
#[test]
fn work_done_does_not_outrank_an_adverse_finding() {
    assert!(
        matches!(
            doc_parity_status(&report(false, &["docs/stub.md"], None)),
            GateStatus::Failed(_)
        ),
        "the guard wrote a stub AND judged the diff insufficient. \
         `AutoUpdated.is_acceptable()` is true, so reporting it here would let \
         the stub certify the gap it is evidence of."
    );
}

/// And a judgement that never arrived is `Errored`, not a verdict either way.
#[test]
fn doc_parity_reports_no_verdict_when_it_obtained_none() {
    assert!(
        matches!(
            doc_parity_status(&report(false, &[], Some("the probe timed out"))),
            GateStatus::Errored(_)
        ),
        "the guard obtained no judgement at all. That is neither a pass nor an \
         accusation, and publishing either claims evidence nobody gathered."
    );
}
