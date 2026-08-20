//! Dependencies enter by decision, not by convenience.
//!
//! Anvil's lockfile is 162 crates against oyatie's 1438. That leanness is a
//! property worth defending: every crate is code nobody here reviewed, running
//! with the daemon's credentials, in a process that clones repositories and
//! spawns agents with `--dangerously-skip-permissions`.
//!
//! These tests are a ratchet, not a ban. The count may fall freely; raising it
//! requires editing this file, which makes the decision visible in review
//! rather than invisible in a lockfile diff.

use std::collections::BTreeSet;
use std::fs;

/// Direct dependencies at the time this ratchet was set.
const DIRECT_DEPENDENCY_CEILING: usize = 22;

/// Transitive crates at the time this ratchet was set.
const LOCKFILE_CEILING: usize = 170;

fn manifest() -> String {
    fs::read_to_string("Cargo.toml").expect("Cargo.toml")
}

fn direct_dependencies() -> BTreeSet<String> {
    let m = manifest();
    let start = match m.find("\n[dependencies]\n") {
        Some(i) => i + "\n[dependencies]\n".len(),
        None => return BTreeSet::new(),
    };
    let rest = &m[start..];
    let end = rest.find("\n[").unwrap_or(rest.len());
    rest[..end]
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split('=').next())
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

#[test]
fn direct_dependency_count_only_falls() {
    let deps = direct_dependencies();
    assert!(
        deps.len() <= DIRECT_DEPENDENCY_CEILING,
        "{} direct dependencies, ceiling is {}. Adding one is a decision: it must be \
         justified here, not merely typed into Cargo.toml.\n{:?}",
        deps.len(),
        DIRECT_DEPENDENCY_CEILING,
        deps
    );
}

#[test]
fn transitive_crate_count_only_falls() {
    let lock = fs::read_to_string("Cargo.lock").expect("Cargo.lock");
    let total = lock.matches("\nname = ").count();
    assert!(
        total <= LOCKFILE_CEILING,
        "{total} crates in the lockfile, ceiling is {LOCKFILE_CEILING}. A dependency's \
         real cost is its transitive closure, which is where a one-line addition becomes \
         forty crates nobody reviewed."
    );
}

#[test]
fn an_admission_policy_exists_and_denies_the_things_that_matter() {
    let deny = fs::read_to_string("deny.toml")
        .expect("deny.toml must exist: dependencies enter by policy, not by habit");

    for (needle, why) in [
        ("yanked", "a yanked crate was withdrawn by its author"),
        (
            "unmaintained",
            "an unmaintained crate accrues unfixed vulnerabilities",
        ),
        ("[licenses]", "license compatibility is not optional"),
        (
            "[bans]",
            "duplicate and wildcard versions are how closures grow",
        ),
    ] {
        assert!(
            deny.contains(needle),
            "deny.toml has no `{needle}` policy — {why}"
        );
    }
}

#[test]
fn the_policy_is_the_same_one_oyatie_uses() {
    // Two policies that drift are worse than one that is occasionally
    // inconvenient. This repository is being absorbed into oyatie; the admission
    // rules should not have to be reconciled at that point.
    let deny = fs::read_to_string("deny.toml").expect("deny.toml");
    assert!(
        deny.contains("oyatie"),
        "deny.toml must record that it is adopted from oyatie, so a reader knows where \
         to change it first"
    );
}
