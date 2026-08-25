//! The 21-gate exemption must be stated on the scorecard, not merely asserted
//! in a doc comment.
//!
//! `report.rs` justifies withholding a verdict from any gate with no registry
//! entry, and defends that exemption with: "is not silent:
//! `fidelity::gap_report().unaudited` publishes its size."
//!
//! `gap_report` had no production caller anywhere in the crate. Its only
//! callers were inside `#[cfg(test)]`. So the size was published nowhere, the
//! exemption was silent, and the sentence justifying it rested on a mechanism
//! that did not run.
//!
//! This is the red half: it fails against a scorecard that does not disclose
//! the count.

use anvil::fidelity;
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use anvil::publish;

/// A certified report. The disclosure belongs on the certified path because
/// that is the common one -- a green verdict is the moment a reader decides
/// whether to trust the score, and what the number omits matters most there.
fn all_passing() -> PreMergeCertificationReport {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();
    let outcomes: Vec<(&str, GateStatus)> =
        names.into_iter().map(|n| (n, GateStatus::Passed)).collect();
    let mut r = PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("the fixture hands over an outcome for every gate in the corpus");
    r.recompute_unmeasured();
    r
}

#[test]
fn the_scorecard_states_how_many_gates_nobody_has_audited() {
    let gaps = fidelity::gap_report(TOTAL_GATES);
    assert!(
        gaps.unaudited > 0,
        "fixture premise: the audit is incomplete, so there is a number to publish"
    );

    let report = all_passing();
    let body = publish::scorecard::render(&report);

    assert!(
        body.contains(&gaps.unaudited.to_string()),
        "the scorecard must state that {} of {} gates have no registry entry; \
         a reader cannot discount what they were never shown:\n{body}",
        gaps.unaudited,
        TOTAL_GATES
    );
}

#[test]
fn the_count_comes_from_the_registry_not_from_a_literal() {
    // If the disclosure were hardcoded it would drift the moment a gate is
    // audited, and say the wrong thing while still looking like evidence.
    let gaps = fidelity::gap_report(TOTAL_GATES);
    assert_eq!(
        gaps.audited + gaps.unaudited,
        TOTAL_GATES,
        "the two halves must partition the corpus"
    );
    assert_eq!(
        gaps.audited,
        fidelity::registry::AUDITED_GATES.len(),
        "audited is the registry's length, not a number written down beside it"
    );
}
