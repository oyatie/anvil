//! The migration ledger must stay true as the code moves under it.
//!
//! 40% of Anvil is superseded by code already in oyatie. That was established
//! once, by audit. A fact like that decays into folklore the moment it lives
//! only in prose -- so it lives as data, and these tests are what keep the data
//! honest as modules are added, split, and renamed.

use anvil::migration::MIGRATION_LEDGER;
use anvil::migration::{deletable_today, surviving_surface, verdict_counts, Confidence, Verdict};
use std::collections::HashSet;
use std::fs;

/// Top-level module names in src/, which is the unit the ledger audits.
fn top_level_modules() -> HashSet<String> {
    let mut out = HashSet::new();
    for entry in fs::read_dir("src").expect("src/ must exist").flatten() {
        let path = entry.path();
        let name = match path.file_stem().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name == "main" || name == "lib" {
            continue;
        }
        if path.is_dir() || path.extension().is_some_and(|e| e == "rs") {
            out.insert(name);
        }
    }
    out
}

#[test]
fn the_ledger_is_not_empty_and_covers_all_three_fates() {
    let (m, r, s, f) = verdict_counts();
    assert!(
        m > 0 && r > 0 && s > 0,
        "a ledger missing a whole verdict class means the audit collapsed a \
         distinction: migrating={m} rewired={r} superseded={s}"
    );
    assert_eq!(MIGRATION_LEDGER.len(), m + r + s + f);
}

#[test]
fn no_component_is_listed_twice_under_conflicting_verdicts() {
    let mut seen: std::collections::HashMap<&str, Verdict> = std::collections::HashMap::new();
    for e in MIGRATION_LEDGER {
        if let Some(prev) = seen.insert(e.component, e.verdict) {
            assert_eq!(
                prev, e.verdict,
                "`{}` appears twice with different verdicts ({:?} then {:?}); a component \
                 cannot have two destinies",
                e.component, prev, e.verdict
            );
        }
    }
}

#[test]
fn a_superseded_verdict_names_the_counterpart_that_supersedes_it() {
    let naked: Vec<&str> = MIGRATION_LEDGER
        .iter()
        .filter(|e| e.verdict == Verdict::Superseded && e.oyatie_counterpart.trim().is_empty())
        .map(|e| e.component)
        .collect();
    assert!(
        naked.is_empty(),
        "these are marked Superseded but name nothing that supersedes them, so nobody can \
         check the claim before deleting working code: {naked:?}"
    );
}

#[test]
fn an_unverified_verdict_never_authorises_deletion() {
    for e in MIGRATION_LEDGER {
        if e.confidence != Confidence::Verified {
            assert!(
                !e.deletion_is_authorised(),
                "`{}` is {:?} yet authorises its own deletion. A probable verdict that \
                 deletes working code is the expensive kind of wrong.",
                e.component,
                e.confidence
            );
        }
    }
    assert!(
        !deletable_today().is_empty(),
        "nothing is deletable, which means the gate is vacuous rather than strict"
    );
}

#[test]
fn every_top_level_module_has_a_verdict() {
    let modules = top_level_modules();
    // Ledger components are recorded as audited (sometimes "foo.rs", "foo (dir)",
    // or a subtree); match on the leading identifier.
    let covered: HashSet<String> = MIGRATION_LEDGER
        .iter()
        .map(|e| {
            e.component
                .split(['/', ' ', '.'])
                .next()
                .unwrap_or("")
                .to_string()
        })
        .collect();

    let missing: Vec<&String> = modules.iter().filter(|m| !covered.contains(*m)).collect();

    assert!(
        missing.is_empty(),
        "{} module(s) have no migration verdict, so nobody knows whether they move, \
         rewire, or are already superseded: {:?}",
        missing.len(),
        missing
    );
}

#[test]
fn the_surviving_surface_is_smaller_than_the_whole() {
    let surviving = surviving_surface().len();
    assert!(
        surviving < MIGRATION_LEDGER.len(),
        "nothing is superseded, which contradicts the audit and would send the naming \
         pass across the whole tree instead of the surface that survives"
    );
}
