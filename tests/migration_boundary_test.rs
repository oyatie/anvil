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

use anvil::migration::{Verdict, check_edge, edge_is_allowed, verdict_for};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Violations present when this gate was written. It may fall; it must not rise.
const KNOWN_VIOLATION_CEILING: usize = 0;

fn module_paths() -> Vec<String> {
    let mut out = Vec::new();
    for entry in fs::read_dir("src").expect("src/").flatten() {
        let p = entry.path();
        let Some(name) = p.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if name == "main" || name == "lib" {
            continue;
        }
        if p.is_dir() || p.extension().is_some_and(|e| e == "rs") {
            out.push(name.to_string());
        }
    }
    out.sort();
    out
}

/// `crate::x` imports, ignoring comments so a doc example cannot fabricate an edge.
fn imports_of(module: &str) -> BTreeSet<String> {
    let dir = Path::new("src").join(module);
    let file = Path::new("src").join(format!("{module}.rs"));
    let mut text = String::new();
    if dir.is_dir() {
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    text.push_str(&fs::read_to_string(&p).unwrap_or_default());
                }
            }
        }
    } else if file.is_file() {
        text = fs::read_to_string(&file).unwrap_or_default();
    }

    let code: String = text
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut out = BTreeSet::new();
    for (i, _) in code.match_indices("crate::") {
        let rest = &code[i + 7..];
        let first: String = rest
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
            .collect();
        if first.is_empty() {
            continue;
        }
        // Capture a second segment too: `crate::pre_merge_guard::report` must
        // resolve to the split-out `pre_merge_guard/report` entry, not to its
        // parent. Only lowercase segments are module paths; a capitalised one
        // is a type.
        let after = &rest[first.len()..];
        if let Some(tail) = after.strip_prefix("::") {
            let second: String = tail
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                .collect();
            if !second.is_empty() {
                out.insert(format!("{first}/{second}"));
                continue;
            }
        }
        out.insert(first);
    }
    out
}

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
fn check_edge_ignores_self_dependency() {
    assert!(check_edge("publish", "publish").is_none());
}

#[test]
#[allow(clippy::absurd_extreme_comparisons)]
fn live_tree_violations_do_not_exceed_the_ratchet() {
    let mut violations = Vec::new();
    for module in module_paths() {
        for dep in imports_of(&module) {
            if let Some(v) = check_edge(&module, &dep) {
                violations.push(v);
            }
        }
    }

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
