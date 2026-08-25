//! Every gate that parses a diff by hand gets the same defects.
//!
//! Thirteen gates were found publishing a file path they had not read out of
//! the diff. Not thirteen mistakes -- one mistake, pasted. Four shapes, all
//! from the same block:
//!
//! | shape                          | example                          |
//! |--------------------------------|----------------------------------|
//! | invented from arbitrary code   | `registry.rs_lookup("a.rs");`    |
//! | plausible fabricated literal   | `.github/workflows/ci.yaml`      |
//! | seeded from `changed_files[0]` | `src/innocent.rs`, a real file   |
//! | empty                          | `""`                             |
//!
//! Six of them additionally read the WHOLE hunk, removals included, and so
//! refused the pull request that DELETED the thing they were looking for.
//!
//! Each was fixed where it was found. That is the N+1 trap: thirteen fixes,
//! one class, and a fourteenth instance is one paste away -- because nothing
//! about writing the fourteenth is harder than writing the thirteenth was.
//!
//! # What this gate does
//!
//! Counts the functions that walk a diff themselves. The count may fall and
//! must never rise. A new gate gets its files from
//! `diff_context::diffs_by_path`, which takes the path from a header that
//! states it and attributes nothing to a hunk that names no file.
//!
//! It is a ratchet rather than a ban because the ban would be a lie today:
//! twenty-five sites exist and they cannot all move in one change. A ban
//! nobody can satisfy gets deleted by the first engineer it blocks, which is
//! how this class survived in the first place.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// Sites present when this gate was written, measured on the tree it landed on.
///
/// LOWER THIS when a change removes one. It is the whole point of the number:
/// a ceiling that is never lowered stops being a ratchet and becomes a budget.
///
/// Counted AFTER the allowlist, because that is how the assertion counts.
/// Setting it to the raw total (25) instead of the counted total (23) left two
/// slack, and a seeded fourteenth parser slipped in under the ceiling without
/// tripping anything -- a gate that was green for a defect it exists to catch.
/// That was found by seeding one, not by reading the code.
const CEILING: usize = 23;

/// Functions allowed to walk a diff, with the reason.
///
/// Deliberately short and deliberately explicit. An allowlist that grows
/// without argument is the same as no gate; each line here should name a
/// reason a reviewer can disagree with.
/// The sanctioned parser is deliberately ABSENT from this list. It does not
/// exist on this branch yet, and `every_allowlist_entry_still_exists_and_still_parses`
/// caught the entry the moment it was written for something not there -- which
/// is the behaviour wanted, so it stays. When `diff_context::diffs_by_path`
/// lands, add it here with the reason "the parser itself" and lower `CEILING`
/// by the number of sites that change removes.
const ALLOWED: &[(&str, &str)] = &[
    (
        "chaos_mutation_guard.rs::touches_rust_source",
        "asks only whether ANY path ends in .rs; it reads no line and \
         attributes no finding, so it has no path to get wrong",
    ),
    (
        "change_delivery/core/purity.rs::diff_is_structure_only",
        "a whole-diff predicate about the change's shape, not a per-file scan; \
         it produces no finding and therefore names no file",
    ),
];

fn rust_sources() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::from("src")];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Every function that walks a unified diff itself.
///
/// The two markers are the ones every copy of the block used: a `+++ b/`
/// header read by hand, or a split on `diff --git`. Test modules are cut off
/// first -- a fixture that spells a diff is not a parser.
fn hand_rolled_parsers() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for path in rust_sources() {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let body = match text.find("#[cfg(test)]") {
            Some(i) => &text[..i],
            None => &text[..],
        };
        let rel = path
            .strip_prefix("src")
            .unwrap_or(&path)
            .display()
            .to_string();

        // Split into functions so the finding names one, rather than the file.
        let starts: Vec<(usize, String)> = body
            .match_indices("fn ")
            .filter(|(i, _)| {
                body[..*i].rsplit('\n').next().is_some_and(|l| {
                    l.trim_start().starts_with("fn ")
                        || l.contains("fn ")
                        || l.trim().is_empty()
                        || l.contains("pub")
                        || l.contains("async")
                })
            })
            .map(|(i, _)| {
                let name: String = body[i + 3..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                (i, name)
            })
            .collect();

        for (idx, (start, name)) in starts.iter().enumerate() {
            let end = starts.get(idx + 1).map(|(s, _)| *s).unwrap_or(body.len());
            let chunk = &body[*start..end];
            if chunk.contains("strip_prefix(\"+++ b/\")") || chunk.contains("split(\"diff --git\")")
            {
                found.insert(format!("{rel}::{name}"));
            }
        }
    }
    found
}

#[test]
fn hand_rolled_diff_parsing_may_fall_but_never_rise() {
    let found = hand_rolled_parsers();

    // Coverage, asserted rather than assumed. A scan that read nothing would
    // otherwise report zero sites and read as a clean tree -- the exact defect
    // this whole class is made of.
    assert!(
        !found.is_empty(),
        "the scan found no diff parsing at all, which means it did not run"
    );

    let allowed: BTreeSet<&str> = ALLOWED.iter().map(|(k, _)| *k).collect();
    let counted: Vec<&String> = found
        .iter()
        .filter(|s| !allowed.contains(s.as_str()))
        .collect();

    assert!(
        counted.len() <= CEILING,
        "{} function(s) parse a diff by hand, ceiling is {CEILING}.\n\
         A new one may not be added: take the files from \
         `diff_context::diffs_by_path`, which reads the path from a header that \
         states it and attributes nothing to a hunk that names no file.\n\
         Thirteen gates published a path they had not read out of the diff, \
         because thirteen places each parsed one.\n  {}",
        counted.len(),
        counted
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn the_ceiling_is_not_a_budget() {
    // A ceiling that is never lowered stops being a ratchet. This does not fail
    // when the count drops -- that would block the very change that improves it
    // -- but it does make the drift visible in the output the moment it opens.
    let allowed: BTreeSet<&str> = ALLOWED.iter().map(|(k, _)| *k).collect();
    let count = hand_rolled_parsers()
        .iter()
        .filter(|s| !allowed.contains(s.as_str()))
        .count();

    if count < CEILING {
        println!(
            "NOTE: {count} hand-rolled diff parsers remain but CEILING is {CEILING}. \
             Lower CEILING to {count} in the change that removed them."
        );
    }
    assert!(count <= CEILING);
}

#[test]
fn every_allowlist_entry_still_exists_and_still_parses() {
    // An allowlist entry for something that no longer parses a diff is a hole
    // held open for nothing, and the next function to take that name inherits
    // the exemption silently.
    let found = hand_rolled_parsers();
    let stale: Vec<&str> = ALLOWED
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !found.contains(*k))
        .collect();
    assert!(
        stale.is_empty(),
        "allowlist entries that no longer parse a diff: {stale:?}\n\
         Remove them, or the exemption outlives the reason for it."
    );
}
