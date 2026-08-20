//! The migration boundary: which direction a dependency may point.
//!
//! # Why this exists
//!
//! A component's verdict in [`crate::migration::registry`] says where it goes.
//! It says nothing about whether it *can* go there. Code marked `Migrating`
//! that imports code marked `Superseded` cannot actually migrate: the thing it
//! depends on is being deleted.
//!
//! That property decays silently. Nobody adds a forbidden import on purpose --
//! they add a `use` for a type that happens to live on the wrong side of a
//! boundary that exists only in a table. Checking it on every diff is what
//! makes the partition survive contact with ordinary work.
//!
//! # The rule
//!
//! ```text
//! Migrating   may depend on: Migrating, Rewired
//! Rewired     may depend on: Migrating, Rewired, Superseded, Scaffolding
//! Superseded  may depend on: anything (it is being deleted)
//! Scaffolding may depend on: anything (the need disappears)
//! ```
//!
//! `Rewired` is deliberately permissive: an adapter's whole job is to sit
//! against the implementation it will later swap out.
//!
//! # What it found
//!
//! Nine violations on first run, and eight of them were one seam. Seven
//! `Migrating` gates imported `crate::pre_merge_guard::report::GateStatus` -- and
//! `GateStatus` was the *only* thing any of them imported. `pre_merge_guard`
//! was marked `Superseded` as a whole, but it is not homogeneous: the
//! evaluator and matrix are superseded by oyatie's gate catalog, while
//! `report.rs` owns the Errored/NotMeasured/is_admissible vocabulary that has
//! no upstream equivalent at all. The gate did not report noise; it located a
//! seam nobody had drawn.

use crate::migration::{MIGRATION_LEDGER, Verdict};

/// One forbidden edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryViolation {
    pub from: String,
    pub from_verdict: Verdict,
    pub to: String,
    pub to_verdict: Verdict,
}

impl BoundaryViolation {
    /// Why this edge blocks migration, in the terms the reader needs.
    pub fn explain(&self) -> String {
        format!(
            "`{}` is {:?} but depends on `{}`, which is {:?}. It cannot migrate while it \
             depends on something being deleted -- either the dependency is misclassified, \
             or the shared code needs splitting out.",
            self.from, self.from_verdict, self.to, self.to_verdict
        )
    }
}

/// Verdict for a module path, preferring the most specific ledger entry.
///
/// `pre_merge_guard/report` must win over `pre_merge_guard`, or splitting a
/// mixed component into its migrating and superseded halves would have no
/// effect on the check.
pub fn verdict_for(module_path: &str) -> Option<Verdict> {
    let normalised = module_path.trim_start_matches("src/").replace('\\', "/");
    let mut best: Option<(usize, Verdict)> = None;

    for entry in MIGRATION_LEDGER {
        let candidate = entry
            .component
            .split(' ')
            .next()
            .unwrap_or("")
            .trim_end_matches(".rs");
        if candidate.is_empty() {
            continue;
        }
        // Only the queried path or an ancestor of it may answer. A CHILD entry
        // must not: asking about `pre_merge_guard` would otherwise be answered
        // by `pre_merge_guard/report`, and the parent's own verdict would be
        // unreachable the moment it was split.
        let matches = normalised == candidate || normalised.starts_with(&format!("{candidate}/"));
        if !matches {
            continue;
        }
        // Longer match = more specific = wins.
        if best.is_none_or(|(len, _)| candidate.len() > len) {
            best = Some((candidate.len(), entry.verdict));
        }
    }
    best.map(|(_, v)| v)
}

/// Whether `from` may depend on `to`.
pub fn edge_is_allowed(from: Verdict, to: Verdict) -> bool {
    match from {
        // Migrating code must not be anchored to code that is going away.
        // Depending on a `Rewired` component IS allowed: its port survives
        // absorption and only the adapter behind it is swapped, so the
        // dependency outlives the migration.
        Verdict::Migrating => matches!(to, Verdict::Migrating | Verdict::Rewired),
        Verdict::Rewired | Verdict::Superseded | Verdict::Scaffolding => true,
    }
}

/// Checks one edge, returning a violation if the direction is forbidden.
pub fn check_edge(from: &str, to: &str) -> Option<BoundaryViolation> {
    if from == to {
        return None;
    }
    let (fv, tv) = (verdict_for(from)?, verdict_for(to)?);
    if edge_is_allowed(fv, tv) {
        return None;
    }
    Some(BoundaryViolation {
        from: from.to_string(),
        from_verdict: fv,
        to: to.to_string(),
        to_verdict: tv,
    })
}
