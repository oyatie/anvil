//! What a passing gate is worth, as published.
//!
//! `gate_proof` knew which gates had been seeded with their own defect and
//! which had only ever been green, and nothing asked it. These cover the
//! answer now that a pull request receives it.

use anvil::gate_proof::{GATE_PROOFS, is_proven, proof_qualifier, unproven_among};

/// The repository-wide figure the qualifier is handed. Supplied by the caller
/// rather than reached for -- `gate_proof` is `Migrating` and the corpus lives
/// in a `Superseded` module -- so these fixtures state it.
const OWING: usize = 23;

/// A gate id the registry really carries, so the tests are not self-referential.
fn a_proven_gate() -> &'static str {
    GATE_PROOFS
        .first()
        .expect("the registry is not empty")
        .gate_id
}

#[test]
fn a_report_whose_passing_gates_are_all_proven_says_nothing() {
    let proven = a_proven_gate();
    assert!(
        proof_qualifier(&[proven], OWING).is_none(),
        "a line with nothing to qualify must not print; one that always prints \
         stops being read"
    );
}

#[test]
fn no_passing_gates_at_all_is_silence_not_a_zero() {
    assert!(proof_qualifier(&[], OWING).is_none());
}

#[test]
fn an_unproven_passing_gate_is_named() {
    let line = proof_qualifier(&["gate_that_owes_a_proof"], OWING).expect("must qualify");
    assert!(
        line.contains("gate_that_owes_a_proof"),
        "the gate must be named, not merely counted -- a count cannot be acted \
         on. Got: {line}"
    );
}

#[test]
fn a_proven_gate_is_not_named_alongside_an_unproven_one() {
    let proven = a_proven_gate();
    let line = proof_qualifier(&[proven, "gate_that_owes_a_proof"], OWING).expect("must qualify");
    assert!(
        !line.contains(proven),
        "naming a gate that HAS demonstrated it can fire is a false accusation, \
         and the defect that made fuzzy matching unusable. Got: {line}"
    );
    assert!(line.contains("1 of 2"), "got: {line}");
}

#[test]
fn the_order_follows_the_report() {
    let got = unproven_among(&["b_gate", "a_gate"]);
    assert_eq!(
        got,
        vec!["b_gate", "a_gate"],
        "the same change twice must read the same way"
    );
}

#[test]
fn is_proven_agrees_with_the_registry() {
    assert!(is_proven(a_proven_gate()));
    assert!(!is_proven("no_such_gate"));
}

#[test]
fn the_published_line_carries_no_source_indentation() {
    let line = proof_qualifier(&["gate_that_owes_a_proof"], OWING).expect("must qualify");
    assert!(
        !line.contains("  "),
        "a wrapped literal without escaped continuations carries its own \
         indentation into the text a person reads. Two instances of this class \
         are already recorded in this repository. Got: {line:?}"
    );
}

#[test]
fn the_qualifier_reaches_a_blocked_scorecard() {
    let rendered = anvil::publish::scorecard::render(&blocked_report());
    assert!(
        rendered.contains("never been seeded"),
        "the module was complete and called by nothing for its whole life; \
         this is the assertion that it is called now.\n{rendered}"
    );
}

/// The whole corpus passing but for one seeded failure.
///
/// A report must cover every gate -- `from_gate_outcomes` refuses a partial
/// corpus, which is the right refusal and caught this fixture's first draft.
/// One gate is failed so the report takes the BLOCKED path, which is the only
/// path the qualifier is published on.
fn blocked_report() -> anvil::pre_merge_guard::PreMergeCertificationReport {
    use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport};
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();
    let outcomes: Vec<(&str, GateStatus)> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let status = if i == 0 {
                GateStatus::Failed("seeded so the report blocks".into())
            } else {
                GateStatus::Passed
            };
            (*n, status)
        })
        .collect();
    PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("an outcome for every gate in the corpus")
}
