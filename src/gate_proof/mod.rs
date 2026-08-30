//! Which gates have demonstrated they can fire, and which have not.
//!
//! # The obligation
//!
//! A check is written by asserting what it should catch, and it passes from the
//! moment it compiles -- so a green tells you nothing about whether it CAN fail.
//! Four checks written in a single session were green for the exact defect they
//! existed to catch, and every one was found by seeding the defect rather than
//! by reading the code.
//!
//! `harness::Rule::fixture` makes that unspellable for anything on the harness:
//! a rule ships a defect it must flag and a twin it must spare, and the harness
//! runs both before trusting a verdict. The seventy-two hand-wired gates are not
//! on the harness, and nothing obliges them to demonstrate anything.
//!
//! This is that obligation, imposed where the harness cannot reach: each gate
//! names the test that seeds its defect and the test that hands it a conformant
//! subject, and both are checked to exist AND to actually exercise that gate.
//!
//! # Why declared rather than inferred
//!
//! The test names follow a convention -- `test_<gate>_red_flag_…`,
//! `test_<gate>_green_…` -- and inferring the mapping from it was tried and
//! abandoned. Fuzzy matching claimed `psa_status` was proven by an ADR test and
//! `slo_status` by the same one. A registry that cites a test which does not
//! exercise the gate is itself green for a defect it exists to catch, which is
//! the class this module is closing. So `exercises` names a symbol the cited
//! test must actually mention, and that is checked.

/// One gate's demonstration that it can fire and that it discriminates.
#[derive(Debug, Clone, Copy)]
pub struct GateProof {
    /// The gate as the certification report names it.
    pub gate_id: &'static str,
    /// A symbol the cited tests must mention -- the guard's type, usually.
    ///
    /// This is what stops a citation being satisfied by any test with a
    /// plausible name. A test that does not name the thing under test is not a
    /// proof of it.
    pub exercises: &'static str,
    /// The test that seeds this gate's defect and asserts it is found.
    pub fires_on: &'static str,
    /// The test that hands it a conformant subject and asserts it is spared.
    ///
    /// Both halves are required. A gate with only the first cannot be shown to
    /// discriminate; one with only the second has never been seen to work.
    pub spares: &'static str,
}

pub mod ledger;
pub use ledger::GATE_PROOFS;

/// The gates that can fire in this deployment and have never been shown to.
///
/// Not every gate can have a demonstration. The gates declared unprovisionable
/// in `admission::ABSENCE_POLICY` -- no telemetry endpoint, no signing backend,
/// no cluster -- cannot be seeded with a defect either, so the obligation is
/// over `absence_blocks`, and this names those still owing it.
///
/// Derived, not written down, for the reason
/// `admission::not_provisioned_count` gives: a corpus-wide literal is a global
/// every lane must edit, and it is what makes a gate migration one
/// unmergeable pull request instead of a series of small ones. The bound is
/// still exact and still one-way, against this change's own merge-base, in
/// `tests/derived_corpus_ratchets_test.rs`.
///
/// The corpus and the predicate are arguments rather than something reached
/// for. `gate_proof` is `Migrating` in `migration::registry` and the corpus
/// lives in `pre_merge_guard`, which is `Superseded`; a module that cannot
/// migrate while it depends on something being deleted has not been made ready
/// to migrate, so the dependency is inverted and the caller supplies both.
pub fn gates_owing_a_proof(
    corpus: &[&'static str],
    can_fire: impl Fn(&str) -> bool,
) -> Vec<&'static str> {
    let proven: std::collections::BTreeSet<&str> = GATE_PROOFS.iter().map(|p| p.gate_id).collect();
    corpus
        .iter()
        .copied()
        .filter(|id| can_fire(id) && !proven.contains(id))
        .collect()
}

/// Whether this gate has demonstrated both halves.
pub fn is_proven(gate_id: &str) -> bool {
    GATE_PROOFS.iter().any(|p| p.gate_id == gate_id)
}

/// Of the gates that passed on this change, those never shown to fire.
///
/// This is the question a green report cannot answer about itself. A gate that
/// passed and has never been seeded with its own defect contributed a tick, not
/// evidence -- and four checks written in one session were green for exactly
/// the defect they existed to catch. Naming them at the point where someone is
/// reading the verdict is the difference between a ledger and a habit.
///
/// Order follows the report, so the same change twice reads the same way.
pub fn unproven_among<'a>(passed: &[&'a str]) -> Vec<&'a str> {
    passed.iter().copied().filter(|id| !is_proven(id)).collect()
}

/// One line qualifying what a set of passing gates is worth.
///
/// `None` when every passing gate is proven -- silence is the correct output
/// for a report with nothing to qualify, and a line that always prints stops
/// being read.
pub fn proof_qualifier(passed: &[&str], owing_repository_wide: usize) -> Option<String> {
    let unproven = unproven_among(passed);
    if unproven.is_empty() {
        return None;
    }
    // Escaped continuations. A wrapped literal without `\` carries its own
    // indentation into the text a person reads -- twice fixed in this
    // repository already, once in `formal_verification` and once in
    // `prevention_debt_line`, which is why it is called out here.
    Some(format!(
        "Proof: {} of {} passing gate(s) have never been seeded with the \
         defect they exist to catch, so their pass is a tick rather than \
         evidence: {}. Repository-wide, {} gate(s) still owe a proof.",
        unproven.len(),
        passed.len(),
        unproven.join(", "),
        owing_repository_wide
    ))
}
