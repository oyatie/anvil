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

/// Direct `[dependencies]` crate names from a Cargo manifest.
///
/// A missing or unreadable `[dependencies]` table is a failed ratchet, not an
/// empty set. The previous line-scan returned `BTreeSet::new()` when it could
/// not find the exact bytes `\n[dependencies]\n`, so a reformatted or
/// CRLF-normalized manifest would pass `deps.len() <= CEILING` with zero crates.
fn direct_dependencies_from(manifest: &str) -> BTreeSet<String> {
    let value: toml::Value = manifest.parse().expect("Cargo.toml must parse as TOML");
    let table = value
        .get("dependencies")
        .and_then(|v| v.as_table())
        .unwrap_or_else(|| {
            panic!(
                "Cargo.toml has no [dependencies] table; an unreadable manifest \
             must fail the ratchet, not pass with zero crates"
            )
        });
    table.keys().cloned().collect()
}

fn direct_dependencies() -> BTreeSet<String> {
    direct_dependencies_from(&manifest())
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

#[test]
fn a_reformatted_dependencies_table_is_still_counted() {
    let deps = direct_dependencies_from(
        "[package]\nname = \"x\"\n[dependencies]\nfoo = \"1\"\nbar = { version = \"2\" }\n",
    );
    assert_eq!(deps, BTreeSet::from(["bar".to_string(), "foo".to_string()]));
}

#[test]
#[should_panic(expected = "no [dependencies] table")]
fn a_manifest_without_dependencies_fails_the_ratchet() {
    let _ = direct_dependencies_from("[package]\nname = \"x\"\n");
}
