//! Drift a production build can establish, from the proof ledger.

use super::*;

/// The drift a production build can actually establish, from the proof ledger.
///
/// Every caller of `audit_against_reality` is inside `#[cfg(test)]`, and
/// `gap_report` hardcodes `drift: Vec::new()`, so the scorecard prints
/// "0 drifting" by construction: a measurement nobody took, published as one
/// that was.
///
/// # Why only one of the three facts is measured, and why that is not a fudge
///
/// `Evidence` carries three: the tool is installed, the gate invokes it, and a
/// seeded-defect fixture exists. Only the third is derivable here --
/// `gate_proof::GATE_PROOFS` IS that ledger. The other two would need a tool
/// name per gate, which the registry does not carry, and a call-graph the
/// binary does not have.
///
/// So the two unmeasurable facts are granted rather than guessed `false`.
/// Guessing `false` would make `observed_fidelity` return `Aspirational` for
/// the whole corpus and report drift against nearly every entry -- dozens of
/// fabricated accusations, which is the symmetric violation of I1 and strictly
/// worse than the hardcoded empty list it replaces. Granting them means the
/// only drift this can report is drift it can prove: a gate declaring
/// `Measured` with no proof behind it.
///
/// That is a real and load-bearing contradiction. `Measured` is the one
/// fidelity that asserts a gate has been SHOWN to fail, and a `Measured` claim
/// with no fixture is exactly the overclaim `FAILURE_PROOFS` exists to catch --
/// now caught in a production report as well as in a test.
pub fn against_the_proof_ledger() -> Vec<FidelityDrift> {
    let proven: std::collections::BTreeSet<&str> = crate::gate_proof::GATE_PROOFS
        .iter()
        .map(|p| p.gate_id)
        .collect();
    audit_against_reality(|gate_id| {
        Some(Evidence {
            // Granted, not measured. See above: guessing `false` fabricates
            // drift, and a fabricated accusation is as much a failure to
            // measure as a missed defect.
            tool_available: true,
            tool_invoked: true,
            failing_fixture_exists: proven.contains(gate_id),
        })
    })
}
