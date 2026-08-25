//! `Measured` is the only fidelity that asserts a gate has been shown to fail
//! on a real defect. Nothing enforced that.
//!
//! `observed_fidelity` requires `failing_fixture_exists` before it will return
//! `Measured`, and `audit_against_reality` exists to catch a gate declaring
//! more than its evidence supports. Neither ever ran outside the test suite:
//! every caller sits inside `#[cfg(test)]`. A gate could declare `Measured`
//! today and no mechanism would contradict it.
//!
//! These tests exercise the validator against synthetic overclaims, not only
//! against the live registry. A validator that can only be run on data which
//! never violates it proves nothing -- that is the shape of every vacuous gate
//! this repository has been removing.

use anvil::fidelity::{
    self, Fidelity, GateFidelity, measured_without_proof, proofs_without_a_gate,
};

fn entry(gate_id: &'static str, fidelity: Fidelity) -> GateFidelity {
    GateFidelity {
        gate_id,
        aspiration: "synthetic",
        reference: "synthetic",
        fidelity,
        gap: "synthetic",
        blocked_on: None,
    }
}

#[test]
fn a_measured_gate_with_no_named_proof_is_reported() {
    let entries = [entry("overclaims_status", Fidelity::Measured)];
    let flagged = measured_without_proof(&entries, &[]);
    assert_eq!(
        flagged,
        vec!["overclaims_status".to_string()],
        "a gate declaring Measured with no proof it can fail must be caught"
    );
}

#[test]
fn a_measured_gate_with_a_named_proof_is_accepted() {
    let entries = [entry("honest_status", Fidelity::Measured)];
    let proofs = [("honest_status", "honest_status_fails_on_a_seeded_defect")];
    assert!(
        measured_without_proof(&entries, &proofs).is_empty(),
        "a named proof satisfies the requirement"
    );
}

#[test]
fn fidelities_below_measured_need_no_proof() {
    // Underclaiming is not a defect. Only `Measured` asserts demonstrated
    // failure, so only `Measured` owes evidence.
    let entries = [
        entry("a_status", Fidelity::Aspirational),
        entry("b_status", Fidelity::Heuristic),
        entry("c_status", Fidelity::Partial),
    ];
    assert!(measured_without_proof(&entries, &[]).is_empty());
}

#[test]
fn a_proof_naming_a_gate_that_does_not_exist_is_reported() {
    let entries = [entry("real_status", Fidelity::Partial)];
    let proofs = [("deleted_status", "some_test")];
    assert_eq!(
        proofs_without_a_gate(&entries, &proofs),
        vec!["deleted_status".to_string()],
        "a proof citing a gate that is gone reads as evidence and backs none"
    );
}

#[test]
fn the_live_registry_declares_no_measured_gate_without_proof() {
    let flagged =
        measured_without_proof(fidelity::registry::AUDITED_GATES, fidelity::FAILURE_PROOFS);
    assert!(
        flagged.is_empty(),
        "these gates declare Measured but name no test proving they can fail: {flagged:?}"
    );
}

#[test]
fn every_named_proof_still_corresponds_to_a_gate() {
    let orphans =
        proofs_without_a_gate(fidelity::registry::AUDITED_GATES, fidelity::FAILURE_PROOFS);
    assert!(
        orphans.is_empty(),
        "proofs naming no live gate: {orphans:?}"
    );
}

#[test]
fn every_named_proof_names_a_test_that_exists() {
    // A proof is a citation. One pointing at a test that does not exist is
    // worse than none, because it reads as evidence.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut corpus = String::new();
    for entry in walk(&root) {
        if entry.extension().is_some_and(|e| e == "rs") {
            corpus.push_str(&std::fs::read_to_string(&entry).unwrap_or_default());
        }
    }
    for (gate, test_name) in fidelity::FAILURE_PROOFS {
        assert!(
            corpus.contains(&format!("fn {test_name}")),
            "{gate} names `{test_name}` as its proof, but no such test exists under tests/"
        );
    }
}

fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}
