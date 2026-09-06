//! Migration destiny per component. See [`registry`] for the ledger itself.

pub mod boundary;
pub mod registry;

pub use boundary::{
    BoundaryViolation, check_edge, edge_is_allowed, ledger_component_identity, verdict_for,
};
pub use registry::{Confidence, MIGRATION_LEDGER, MigrationEntry, Verdict};

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

/// Scans the live tree for forbidden dependency edges.
///
/// Returns `Err(reason)` when the source tree cannot be read, so the caller can
/// report `NotMeasured` rather than an absence of violations. A gate that
/// cannot see the code must not report that the code is clean.
pub fn live_tree_violations(
    repo_root: &crate::git_manager::SubjectRoot,
) -> Result<Vec<BoundaryViolation>, String> {
    let repo_root = repo_root.as_path();
    let mut out = Vec::new();
    for (module, deps) in crate::source_scan::paths::production_module_dependencies(repo_root)? {
        for dep in deps {
            if let Some(v) = check_edge(&module, &dep) {
                out.push(v);
            }
        }
    }
    Ok(out)
}
