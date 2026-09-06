//! The ledger of gates that claim `Measured`, and the proof each one names.
//!
//! Kept beside the fidelity model rather than inside it. The model answers what
//! a gate measures; this answers who has shown that it can fail, which is a
//! separate obligation with a separate failure mode -- a citation that reads as
//! evidence while backing none.

use super::{Fidelity, GateFidelity};

/// Gates that declare `Measured`, each naming the test that proves it can fail.
///
/// `Measured` is the only fidelity that asserts a gate has been shown to fail on
/// a real defect. `observed_fidelity` already requires `failing_fixture_exists`,
/// but nothing ever supplied that evidence in production: every caller of
/// `audit_against_reality` lives inside `#[cfg(test)]`, so a gate could declare
/// `Measured` and no mechanism outside the test suite would contradict it.
///
/// A separate table rather than a field on `GateFidelity`: only gates claiming
/// `Measured` need one, and adding a field would touch every entry in
/// `registry::AUDITED_GATES` -- a file every gate pull request already conflicts
/// on. The count is `AUDITED_GATES.len()` and is not restated here; this
/// sentence said "fifty-one" while the table held fifty-four.
///
/// The ratio rises when code closes a named gap, not when a test is written.
/// A gate with seeded-defect fixtures and a wired production path stays
/// `Partial` for as long as its own gap records an unimplemented rule; the row
/// below is added when the rule exists and the fixture shows it firing.
pub const FAILURE_PROOFS: &[(&str, &str)] = &[
    (
        "unresolved_review_status",
        "an_unresolved_thread_is_reported",
    ),
    (
        "shape_status",
        "adapter_naming_fires_on_an_adapter_that_names_no_port_of_its_unit",
    ),
];

/// The smallest number of `Measured` gates this tree is allowed to have.
///
/// A floor, not a pin. It rises when a gate closes its gap and names the proof
/// that it can fail; it may never fall, because a gate that was shown to
/// measure something does not stop having been shown it. Lowering this number
/// is how a regression would be made to look like a passing build, so it is the
/// line a review should stop at.
pub const MEASURED_GATES_FLOOR: usize = 2;

/// Gate ids declaring `Measured` with no named proof that they can fail.
///
/// Takes the entries rather than reading the const so it can be exercised
/// against a synthetic overclaim; a validator that can only be run on data
/// which never violates it is not a validator.
pub fn measured_without_proof(entries: &[GateFidelity], proofs: &[(&str, &str)]) -> Vec<String> {
    entries
        .iter()
        .filter(|e| e.fidelity == Fidelity::Measured)
        .filter(|e| !proofs.iter().any(|(gate, _)| *gate == e.gate_id))
        .map(|e| e.gate_id.to_string())
        .collect()
}

/// Named proofs that do not correspond to any registry entry.
///
/// The symmetric check. A proof naming a gate that no longer exists is a
/// citation to nothing, and reads as evidence while backing none.
pub fn proofs_without_a_gate(entries: &[GateFidelity], proofs: &[(&str, &str)]) -> Vec<String> {
    proofs
        .iter()
        .filter(|(gate, _)| !entries.iter().any(|e| e.gate_id == *gate))
        .map(|(gate, _)| (*gate).to_string())
        .collect()
}
