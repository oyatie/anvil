//! The line a scorecard prints about what a passing gate is worth.
//!
//! Its own module because it is a join, and the join has a constraint. The
//! ledger lives in `gate_proof`, which is `Migrating` in `migration::registry`;
//! the corpus and the "can this gate fire in this deployment" predicate live in
//! `pre_merge_guard`, which is `Superseded`. A module that cannot migrate while
//! it depends on something being deleted has not been made ready to migrate, so
//! `gate_proof` takes both as arguments and the two are brought together here,
//! in a unit that is `Rewired` and may depend on either.

/// How many gates in this corpus still owe a demonstration that they can fire.
pub fn gates_owing() -> usize {
    let ids: Vec<&str> = crate::pre_merge_guard::matrix::GATE_LABELS
        .iter()
        .map(|(id, _, _)| *id)
        .collect();
    crate::gate_proof::gates_owing_a_proof(&ids, crate::pre_merge_guard::admission::absence_blocks)
        .len()
}

/// The qualifier for the gates that passed on this change, or nothing to say.
pub fn qualifier(passed: &[&str]) -> Option<String> {
    crate::gate_proof::proof_qualifier(passed, gates_owing())
}

#[cfg(test)]
mod tests {
    /// A gate the ledger does not cover, so the figure must react to it.
    const UNCOVERED: &str = "a_gate_no_ledger_row_covers";

    /// The figure is a join, not a literal: adding one uncovered gate to the
    /// corpus raises it by exactly one.
    ///
    /// It deliberately does not assert the figure is above zero. Zero is the
    /// goal, and a fixture that forbids reaching it is a check that fails when
    /// the work succeeds. What zero must not be allowed to mean is "the corpus
    /// was empty", so that is what is asserted instead -- the same distinction
    /// I1 draws between measuring nothing and finding nothing.
    #[test]
    fn the_repository_wide_figure_is_a_join_and_not_a_literal() {
        let mut corpus: Vec<&'static str> = crate::pre_merge_guard::matrix::GATE_LABELS
            .iter()
            .map(|(id, _, _)| *id)
            .collect();
        assert!(
            !corpus.is_empty(),
            "an empty corpus makes every gate look proven"
        );

        let owing = super::gates_owing();
        assert!(
            owing < crate::pre_merge_guard::report::TOTAL_GATES,
            "{owing} exceeds the corpus it is drawn from, so it is not this join"
        );

        corpus.push(UNCOVERED);
        let probe =
            |id: &str| id == UNCOVERED || crate::pre_merge_guard::admission::absence_blocks(id);
        assert_eq!(
            crate::gate_proof::gates_owing_a_proof(&corpus, probe).len(),
            owing + 1,
            "a gate with no ledger row did not raise the figure, so the figure \
             is not read from the ledger"
        );
    }
}
