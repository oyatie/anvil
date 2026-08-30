//! A gate declaring `Measured` with no proof behind it, bounded and falling.
//!
//! `fidelity::drift::against_the_proof_ledger` is the production measurement:
//! it joins the declared fidelity of every audited gate against
//! `gate_proof::GATE_PROOFS`, and reports the one contradiction it can prove --
//! a gate claiming to have been SHOWN to fail with no fixture that shows it.
//!
//! The plan bounds it here rather than freezing a number, for the reason the
//! derived corpus ratchets give: a literal is a global every lane must edit,
//! and it is what makes a migration one unmergeable pull request instead of a
//! series of small ones. The bound is the merge-base's own count.

/// The instrument must be able to express a non-zero answer.
///
/// Without this the bound below passes on a `against_the_proof_ledger` that
/// returned an empty vec for any reason at all -- a scan that cannot report a
/// finding reports a pass, which is the defect this file is one of many
/// guarding against.
#[test]
fn the_drift_measurement_can_report_a_gate_that_overclaims() {
    let drift = anvil::fidelity::audit_against_reality(|_| {
        Some(anvil::fidelity::Evidence {
            tool_available: true,
            tool_invoked: true,
            // Nothing is proven. Every `Measured` declaration is then an
            // overclaim, and a measurement that still reports none is not
            // measuring.
            failing_fixture_exists: false,
        })
    });
    let measured = anvil::fidelity::registry::AUDITED_GATES
        .iter()
        .filter(|g| matches!(g.fidelity, anvil::fidelity::Fidelity::Measured))
        .count();
    assert_eq!(
        drift.len(),
        measured,
        "with no proof anywhere, every one of the {measured} `Measured` \
         declaration(s) is an overclaim and the measurement reported \
         {} -- so it is not reading the ledger",
        drift.len()
    );
}

/// A `Measured` declaration with no proof row is an overclaim, and the bound is
/// zero rather than the merge-base's count.
///
/// The derived ratchets in this tree bound against the merge-base because their
/// subjects are debt being paid down: a count that must fall, from wherever it
/// stands. This one is not debt. `Measured` is the single fidelity that asserts
/// a gate has been SHOWN to fail, and a gate asserting it with no fixture
/// behind it is not partway to correct -- it is the exact claim the proof
/// ledger exists to refuse. There is no number of them that is acceptable
/// today, so freezing today's number would license the next one.
#[test]
fn no_gate_declares_measured_without_a_proof_behind_it() {
    let drift = anvil::fidelity::drift::against_the_proof_ledger();
    assert!(
        drift.is_empty(),
        "{} gate(s) declare `Measured` with no row in `GATE_PROOFS`:\n  {}\n\
         `Measured` asserts the gate has been shown to fail. Either add the \
         seeded-defect fixture and its ledger row, or declare the fidelity the \
         gate actually has.",
        drift.len(),
        drift
            .iter()
            .map(|d| d.gate_id.to_string())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}
