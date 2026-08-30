//! A cited proof must exist, and must actually exercise the gate it is cited for.
//!
//! Inference by naming convention was tried first and produced FALSE PROOFS:
//! it claimed `psa_status` was demonstrated by an ADR test, and `slo_status` by
//! the same one. A registry that cites a test which does not touch the gate is
//! itself green for the defect it exists to catch -- the class it is closing.
//!
//! So every row is checked twice: the test exists, and its body names the thing
//! under test.

use anvil::gate_proof::{GATE_PROOFS, GateProof, gates_owing_a_proof};
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
        // `async fn` as well as `fn`. Matching only the latter made every
        // `#[tokio::test]` in the suite invisible, so a proof citing one was
        // reported as naming a test that does not exist -- an accusation drawn
        // from the scan's own blind spot, which is I1 in the direction that
        // refuses honest work rather than the one that certifies dishonest work.
        let mut marks: Vec<(usize, usize)> = Vec::new();
        for (i, _) in src.match_indices("\nfn ") {
            marks.push((i + 1, 3));
        }
        for (i, _) in src.match_indices("\nasync fn ") {
            marks.push((i + 1, 9));
        }
        marks.sort_unstable();
        for (idx, (start, skip)) in marks.iter().enumerate() {
            let end = marks.get(idx + 1).map(|(s, _)| *s).unwrap_or(src.len());
            let chunk = &src[*start..end];
            let name: String = chunk[*skip..]
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

/// The set is what the corpus says, and nothing else says it.
///
/// The one-way direction lives in `tests/derived_corpus_ratchets_test.rs`,
/// against this change's own merge-base. What is left here is that the library
/// function computes the same set this test would compute by hand — so the
/// ratchet cannot be satisfied by a function that has quietly stopped counting.
#[test]
fn the_gates_owing_a_proof_are_the_unproven_ones_that_can_fire() {
    let proven: BTreeSet<&str> = GATE_PROOFS.iter().map(|p: &GateProof| p.gate_id).collect();
    let by_hand: Vec<&str> = every_gate_id()
        .into_iter()
        .filter(|id| absence_blocks(id) && !proven.contains(id))
        .collect();

    assert_eq!(
        gates_owing_a_proof(&every_gate_id(), absence_blocks,),
        by_hand,
        "the set must be derived from the corpus and the proof ledger"
    );
    assert!(
        !every_gate_id().is_empty(),
        "fixture sanity: an empty corpus makes every gate look proven, so the \
         equality above would hold between two empty sets"
    );

    // Non-vacuity by construction rather than by requiring the count to be
    // above zero. Zero is the goal; a fixture that fails when the work
    // succeeds is not measuring the work.
    let mut with_one_more = every_gate_id();
    with_one_more.push("a_gate_no_ledger_row_covers");
    assert_eq!(
        gates_owing_a_proof(&with_one_more, |id| id == "a_gate_no_ledger_row_covers"
            || absence_blocks(id))
        .len(),
        by_hand.len() + 1,
        "a gate with no ledger row did not enter the set, so the set is not \
         read from the ledger"
    );
}

/// The scan must see an async test, or a proof citing one reads as missing.
///
/// Named rather than counted: the four proofs this caught were all
/// `#[tokio::test]`, and reverting the scan to bare `fn` makes this fail
/// instead of quietly accusing them.
#[test]
fn the_scan_finds_an_async_test() {
    let bodies = test_bodies();
    assert!(
        body_of(
            &bodies,
            "monorepo_fires_on_an_agent_scratch_directory_in_a_commit"
        )
        .is_some(),
        "the scan did not find a known `#[tokio::test]`, so every proof citing \
         one would be reported as naming a test that does not exist"
    );
}
