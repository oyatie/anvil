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
    /// The repository-wide figure is measured, not written down: it falls when
    /// a gate is proven and rises when one is added without a proof, and
    /// `tests/derived_corpus_ratchets_test.rs` bounds the direction.
    #[test]
    fn the_repository_wide_figure_is_derived_and_non_trivial() {
        let owing = super::gates_owing();
        assert!(
            owing > 0 && owing < crate::pre_merge_guard::report::TOTAL_GATES,
            "{owing} is not a count of the gates that can fire and have no proof"
        );
    }
}
