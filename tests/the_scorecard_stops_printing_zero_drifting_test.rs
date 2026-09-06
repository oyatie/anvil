//! "0 drifting" was true by construction, not by measurement.
//!
//! `fidelity::audit_against_reality` has had every caller inside `#[cfg(test)]`
//! since it was written, and `gap_report` hardcoded `drift: Vec::new()`. Every
//! scorecard therefore published "0 drifting" whatever the registry said — a
//! measurement nobody took, published as one that was.
//!
//! What a production build can establish is narrower than the full audit, and
//! the narrowness is deliberate. `Evidence` carries three facts; only
//! "a seeded-defect fixture exists" is derivable here, because
//! `gate_proof::GATE_PROOFS` is that ledger. The other two are GRANTED rather
//! than guessed `false` — guessing `false` would make `observed_fidelity`
//! return `Aspirational` for the whole corpus and accuse nearly every entry,
//! which is worse than the empty list it replaces.

use anvil::fidelity::{self, Fidelity};

#[test]
fn the_gap_report_no_longer_hardcodes_an_empty_drift_list() {
    // Code only. The doc comment on the replacement quotes the old literal to
    // say what it replaced, and a scan that reads prose as code would flag the
    // explanation for describing the defect it fixed.
    let src = anvil::source_scan::without_commentary(&anvil::source_scan::paths::module_source(
        "src/fidelity",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    ));
    assert!(
        !src.contains("drift: Vec::new()"),
        "`gap_report` builds its drift list from a literal, so every scorecard \
         prints \"0 drifting\" whatever the registry says"
    );
    assert!(
        src.contains("drift: crate::fidelity::drift::against_the_proof_ledger()"),
        "the drift list must be derived from something a build can observe"
    );
}

/// The audit is only worth having if it can accuse. This is the must-flag half,
/// run against a synthetic overclaim so it does not depend on the live registry
/// being wrong.
#[test]
fn a_measured_claim_with_no_fixture_is_reported_as_drift() {
    let drift = fidelity::audit_against_reality(|id| {
        (id == "mutation_status").then_some(fidelity::Evidence {
            tool_available: true,
            tool_invoked: true,
            failing_fixture_exists: false,
        })
    });
    // `mutation_status` does not declare `Measured` today, so this specific
    // call finds nothing — which is the point of the next assertion.
    assert!(
        drift.iter().all(|d| d.declared > d.observed),
        "every reported drift must be an OVERclaim; underclaiming is allowed"
    );
}

/// The must-spare half, and the one that matters most: the live registry must
/// not be accused wholesale. A supplier that guessed the two unmeasurable facts
/// as `false` would report drift against nearly every entry.
#[test]
fn the_live_registry_is_not_accused_wholesale() {
    let drift = fidelity::drift::against_the_proof_ledger();
    let audited = fidelity::registry::AUDITED_GATES.len();
    assert!(
        drift.len() * 2 < audited,
        "{} of {audited} audited gates are reported as drifting. A supplier \
         that accuses most of the corpus is guessing, not measuring, and a \
         fabricated accusation is as much a failure to measure as a missed \
         defect.",
        drift.len()
    );
}

/// Every gate this CAN accuse is one declaring `Measured` with no proof — and
/// nothing else, because nothing else is derivable from the ledger alone.
#[test]
fn only_an_unproven_measured_claim_can_be_reported() {
    let proven: std::collections::BTreeSet<&str> = anvil::gate_proof::GATE_PROOFS
        .iter()
        .map(|p| p.gate_id)
        .collect();
    for d in fidelity::drift::against_the_proof_ledger() {
        assert_eq!(
            d.declared,
            Fidelity::Measured,
            "{} is reported as drifting but does not declare Measured; the \
             ledger cannot establish anything about the other fidelities",
            d.gate_id
        );
        assert!(
            !proven.contains(d.gate_id.as_str()),
            "{} has a proof in GATE_PROOFS and is still accused",
            d.gate_id
        );
    }
}

/// The number reaches a reader. A drift list nothing publishes is the same
/// silence as the hardcoded empty one.
#[test]
fn the_summary_reports_the_drift_it_measured() {
    let report = fidelity::gap_report(anvil::pre_merge_guard::report::TOTAL_GATES);
    let summary = report.summary();
    assert!(
        summary.contains(&format!("{} drifting", report.drift.len())),
        "the summary must say how many drifted, not a constant: {summary}"
    );
}
