//! Migration destiny per component. See [`registry`] for the ledger itself.

pub mod boundary;
pub mod registry;

pub use boundary::{check_edge, edge_is_allowed, verdict_for, BoundaryViolation};
pub use registry::{Confidence, MigrationEntry, Verdict, MIGRATION_LEDGER};

/// Counts by verdict. Returns (migrating, rewired, superseded, scaffolding).
pub fn verdict_counts() -> (usize, usize, usize, usize) {
    let mut m = 0;
    let mut r = 0;
    let mut s = 0;
    let mut f = 0;
    for e in MIGRATION_LEDGER {
        match e.verdict {
            Verdict::Migrating => m += 1,
            Verdict::Rewired => r += 1,
            Verdict::Superseded => s += 1,
            Verdict::Scaffolding => f += 1,
        }
    }
    (m, r, s, f)
}

/// Components that survive absorption in some form -- the only ones worth
/// renaming or restructuring. Superseded and scaffolding code is both deleted,
/// so applying the naming law to either is waste.
pub fn surviving_surface() -> Vec<&'static MigrationEntry> {
    MIGRATION_LEDGER
        .iter()
        .filter(|e| !matches!(e.verdict, Verdict::Superseded | Verdict::Scaffolding))
        .collect()
}

/// Superseded components whose evidence is strong enough to act on. Deliberately
/// narrower than "everything marked Superseded": a probable verdict must not
/// delete working code.
pub fn deletable_today() -> Vec<&'static MigrationEntry> {
    MIGRATION_LEDGER
        .iter()
        .filter(|e| e.deletion_is_authorised())
        .collect()
}
