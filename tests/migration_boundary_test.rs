//! Migrating code must not be anchored to code that is being deleted.
//!
//! The migration ledger says where each component goes. It says nothing about
//! whether it *can* go there. A component marked `Migrating` that imports one
//! marked `Superseded` cannot migrate — the thing it depends on will not exist.
//!
//! Nobody adds a forbidden import deliberately. They add a `use` for a type
//! that happens to sit on the far side of a boundary which exists only in a
//! table. Checking every diff is what keeps the partition true.
//!
//! This runs WARN-ONLY against the live tree: the current violation count is
//! ratcheted, so it can shrink but never grow. Turning it hard-fail before the
//! known seams are cut would block ordinary work for a problem already
//! recorded.

use anvil::git_manager::{SubjectRoot, Uncloned};
use anvil::migration::{Verdict, check_edge, edge_is_allowed, live_tree_violations, verdict_for};

/// Violations present when this gate was written. It may fall; it must not rise.
const KNOWN_VIOLATION_CEILING: usize = 0;

#[test]
fn the_rule_is_strict_only_where_it_must_be() {
    // Migrating is the one verdict that constrains: it may depend only on Migrating.
    assert!(edge_is_allowed(Verdict::Migrating, Verdict::Migrating));
    assert!(!edge_is_allowed(Verdict::Migrating, Verdict::Superseded));
    assert!(!edge_is_allowed(Verdict::Migrating, Verdict::Scaffolding));
    // Allowed: a Rewired component's port survives; only its adapter is swapped.
    assert!(edge_is_allowed(Verdict::Migrating, Verdict::Rewired));

    // An adapter's whole job is to sit against what it will later swap out.
    assert!(edge_is_allowed(Verdict::Rewired, Verdict::Superseded));
    assert!(edge_is_allowed(Verdict::Superseded, Verdict::Migrating));
}

#[test]
fn a_more_specific_ledger_entry_beats_a_broader_one() {
    // The whole point of splitting a mixed component: pre_merge_guard is
    // Superseded, but pre_merge_guard/report is the admission vocabulary and
    // migrates. If the broader entry won, splitting would change nothing.
    assert_eq!(verdict_for("pre_merge_guard"), Some(Verdict::Superseded));
    assert_eq!(
        verdict_for("pre_merge_guard/report"),
        Some(Verdict::Migrating),
        "the specific entry must win, or a mixed component can never be split"
    );
}

#[test]
fn extracted_gate_status_keeps_the_existing_vocabulary_custody() {
    for component in ["pre_merge_guard/status", "pre_merge_guard/report"] {
        assert_eq!(
            verdict_for(component),
            Some(Verdict::Migrating),
            "{component}"
        );
    }
    for component in ["pre_merge_guard", "pre_merge_guard/evaluator"] {
        assert_eq!(
            verdict_for(component),
            Some(Verdict::Superseded),
            "{component}"
        );
    }
}

#[test]
fn check_edge_ignores_self_dependency() {
    assert!(check_edge("publish", "publish").is_none());
}

#[test]
#[allow(clippy::absurd_extreme_comparisons)]
fn live_tree_violations_do_not_exceed_the_ratchet() {
    let subject = SubjectRoot::asserted(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        Uncloned::SelfMeasurement,
    );
    let violations = live_tree_violations(&subject).expect("the live source tree is readable");

    let rendered: Vec<String> = violations.iter().map(|v| v.explain()).collect();
    // The ceiling is a ratchet that happens to stand at zero today, having come
    // down from nine. Comparing against it rather than asserting `is_empty()`
    // keeps that intent legible: if a seam is found that genuinely cannot be
    // cut yet, the ceiling is raised deliberately and visibly, not by deleting
    // the check.
    #[allow(clippy::absurd_extreme_comparisons)]
    let within_ratchet = violations.len() <= KNOWN_VIOLATION_CEILING;
    assert!(
        within_ratchet,
        "{} migration-boundary violation(s), ceiling is {}. This ratchet may fall, never \
         rise: a new edge from Migrating code into Superseded code means that component \
         can no longer migrate.\n{}",
        violations.len(),
        KNOWN_VIOLATION_CEILING,
        rendered.join("\n")
    );
}
