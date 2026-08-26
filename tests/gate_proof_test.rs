//! A cited proof must exist, and must actually exercise the gate it is cited for.
//!
//! Inference by naming convention was tried first and produced FALSE PROOFS:
//! it claimed `psa_status` was demonstrated by an ADR test, and `slo_status` by
//! the same one. A registry that cites a test which does not touch the gate is
//! itself green for the defect it exists to catch -- the class it is closing.
//!
//! So every row is checked twice: the test exists, and its body names the thing
//! under test.

use anvil::gate_proof::{GATE_PROOFS, GATES_WITHOUT_PROOF, GateProof};
use anvil::pre_merge_guard::admission::absence_blocks;
use anvil::pre_merge_guard::report::PreMergeCertificationReport;
use anvil::source_scan::without_commentary;
use std::collections::BTreeSet;
use std::fs;

/// Every test function in the suite, with the code of its body.
fn test_bodies() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for entry in fs::read_dir("tests").into_iter().flatten().flatten() {
        let p = entry.path();
        if !p.extension().is_some_and(|e| e == "rs") {
            continue;
        }
        // Code, not commentary: a test whose COMMENT mentions a guard has not
        // exercised it, and citing one would be the same fabrication the
        // registry exists to prevent.
        let src = without_commentary(&fs::read_to_string(&p).unwrap_or_default());
        let marks: Vec<usize> = src.match_indices("\nfn ").map(|(i, _)| i + 1).collect();
        for (idx, start) in marks.iter().enumerate() {
            let end = marks.get(idx + 1).copied().unwrap_or(src.len());
            let chunk = &src[*start..end];
            let name: String = chunk[3..]
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !name.is_empty() {
                out.push((name, chunk.to_string()));
            }
        }
    }
    out
}

fn body_of<'a>(bodies: &'a [(String, String)], name: &str) -> Option<&'a str> {
    bodies
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, b)| b.as_str())
}

fn every_gate_id() -> Vec<&'static str> {
    PreMergeCertificationReport::unmeasured("enumerating")
        .named_statuses()
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn every_cited_test_exists() {
    let bodies = test_bodies();
    let mut missing = Vec::new();
    for p in GATE_PROOFS {
        for cited in [p.fires_on, p.spares] {
            if body_of(&bodies, cited).is_none() {
                missing.push(format!("{}: `{cited}`", p.gate_id));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "{} cited proof(s) name a test that does not exist:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}

#[test]
fn every_cited_test_actually_exercises_its_gate() {
    // The check that makes the registry mean something. A plausible name is not
    // a proof; the test has to name the thing under test, in code.
    let bodies = test_bodies();
    let mut bogus = Vec::new();
    for p in GATE_PROOFS {
        for cited in [p.fires_on, p.spares] {
            if let Some(body) = body_of(&bodies, cited)
                && !body.contains(p.exercises)
            {
                bogus.push(format!(
                    "{}: `{cited}` never mentions `{}`",
                    p.gate_id, p.exercises
                ));
            }
        }
    }
    assert!(
        bogus.is_empty(),
        "{} cited proof(s) do not exercise the gate they are cited for. This is \
         how inference produced a registry claiming `psa_status` was proven by \
         an ADR test:\n  {}",
        bogus.len(),
        bogus.join("\n  ")
    );
}

#[test]
fn a_proof_cites_two_different_tests() {
    // Both halves, and not the same one twice. A gate with only a red case
    // cannot be shown to discriminate; with only a green case it has never been
    // seen to work at all.
    for p in GATE_PROOFS {
        assert_ne!(
            p.fires_on, p.spares,
            "{}: cites one test as both halves, which demonstrates neither",
            p.gate_id
        );
    }
}

#[test]
fn every_proof_names_a_real_gate_and_names_it_once() {
    let real: BTreeSet<&str> = every_gate_id().into_iter().collect();
    let mut seen = BTreeSet::new();
    for p in GATE_PROOFS {
        assert!(
            real.contains(p.gate_id),
            "{} is not a gate in the corpus",
            p.gate_id
        );
        assert!(seen.insert(p.gate_id), "{} is recorded twice", p.gate_id);
    }
}

#[test]
fn gates_owing_a_demonstration_may_fall_but_never_rise() {
    // The obligation, as a number. A gate that cannot fire in this deployment
    // cannot be seeded with a defect either, so the count is over the gates
    // that CAN -- `absence_blocks` is exactly that set.
    let proven: BTreeSet<&str> = GATE_PROOFS.iter().map(|p: &GateProof| p.gate_id).collect();
    let owing: Vec<&str> = every_gate_id()
        .into_iter()
        .filter(|id| absence_blocks(id) && !proven.contains(id))
        .collect();

    assert_eq!(
        owing.len(),
        GATES_WITHOUT_PROOF,
        "{} gate(s) can fire in this deployment and have not demonstrated that they do; \
         the ledger records {GATES_WITHOUT_PROOF}.\n\
         If this ROSE, a gate was added without a seeded defect -- it cannot be believed \
         until it has one.\n\
         If this FELL, lower the constant in the change that proved the gate.\n  {}",
        owing.len(),
        owing.join("\n  ")
    );
}
