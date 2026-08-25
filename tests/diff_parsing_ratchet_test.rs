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

/// Sites present when this gate was written, counted AFTER the allowlist.
///
/// EXACT, not a ceiling. `<=` leaves slack, and slack is what let a seeded
/// fourteenth parser through the first draft of this gate untripped: the
/// constant was the raw total while the assertion counted the allowlisted set,
/// so two sites of headroom existed and nothing said so.
///
/// Exact also removes the need for a reminder to lower it. That reminder was a
/// `println!`, which `cargo test` captures and hides on a passing test -- anvil
/// reviewed this gate and pointed out the note would never be seen. A number
/// that must be updated by the change that moves it needs no reminder.
const CEILING: usize = 19;

/// Functions allowed to walk a diff, with the reason.
///
/// Deliberately short and deliberately explicit. An allowlist that grows
/// without argument is the same as no gate; each line here should name a
/// reason a reviewer can disagree with.
/// `diffs_by_path` is the sanctioned parser and heads this list.
///
/// It was deliberately absent while it did not yet exist, and
/// `every_allowlist_entry_still_exists_and_still_parses` caught the entry the
/// moment it was written for something not there. That is the behaviour wanted,
/// and it is why the entry could be added only by the change that brings the
/// parser -- which is this one.
const ALLOWED: &[(&str, &str)] = &[
    (
        "git_manager/diff_context.rs::diffs_by_path",
        "the parser itself: the one place this parsing is allowed to live. It \
         takes the path from a header that STATES it -- `+++ b/` or \
         `diff --git ... b/` -- and attributes nothing to a hunk that names no \
         file, which is what the thirteen gates it replaces all got wrong",
    ),
    (
        "chaos_mutation_guard.rs::touches_rust_source",
        "asks only whether ANY path ends in .rs; it reads no line and \
         attributes no finding, so it has no path to get wrong",
    ),
    (
        "harness/rules.rs::fixture",
        "CONSTRUCTS a diff, it does not read one: `Rule::fixture` builds the \
         seeded defect and its conformant twin, and a fixture that spells a \
         `+++ b/` header contains the literal without parsing anything. This is \
         the cost of matching the literal rather than the call syntax -- the fix \
         for the evasion vectors anvil's own review found -- and it is paid \
         here, once, by name",
    ),
    (
        "change_delivery/core/purity.rs::diff_is_structure_only",
        "a whole-diff predicate about the change's shape, not a per-file scan; \
         it produces no finding and therefore names no file",
    ),
];

/// The strings that only appear in code reading a unified diff by hand.
///
/// The LITERAL, not the method called on it. Anvil's own review of this gate
/// found the first draft keyed on `strip_prefix("+++ b/")` and
/// `split("diff --git")` exactly, so `starts_with("+++ b/")`, `split_once`, a
/// raw string or a regex all walked straight past. Matching the literal closes
/// every one of those at once, and a raw string `r"+++ b/"` contains it too.
const DIFF_MARKERS: &[&str] = &["+++ b/", "diff --git"];

/// Everything that can introduce a function definition.
///
/// Also from that review: the first draft keyed on `pub`/`async` and dropped a
/// bare `unsafe fn` or `extern "C" fn` entirely, so a parser written either way
/// was invisible to the gate that exists to see it.
const FN_INTRODUCERS: &[&str] = &[
    "fn ",
    "pub fn ",
    "pub(crate) fn ",
    "async fn ",
    "pub async fn ",
    "pub(crate) async fn ",
    "unsafe fn ",
    "pub unsafe fn ",
    "const fn ",
    "pub const fn ",
    "extern ",
    "pub extern ",
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

/// Removes `#[cfg(test)]` items by matching braces.
///
/// The first draft truncated the file at the first occurrence of the string.
/// Anvil's review pointed out that a doc comment, an inner module, or a string
/// literal carrying that text near the top of a file blinds the scan to every
/// production function below it -- a gate that silently stops looking, which is
/// the defect this whole class is made of.
fn without_test_modules(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("#[cfg(test)]") {
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        let Some(open) = after.find('{') else {
            return out;
        };
        let mut depth = 0i32;
        let mut end = None;
        for (k, c) in after[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(open + k + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        match end {
            Some(e) => rest = &after[e..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// A path written with `/` on every platform.
///
/// `display()` emits `\` on Windows, so every POSIX key in `ALLOWED` would fail
/// to match there and the exemptions would evaporate silently.
fn posix_rel(path: &std::path::Path) -> String {
    path.strip_prefix("src")
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// The byte offsets at which a function definition begins, with its name.
fn function_starts(body: &str) -> Vec<(usize, String)> {
    let mut out: Vec<(usize, String)> = Vec::new();
    for (off, line) in line_offsets(body) {
        let t = line.trim_start();
        if !FN_INTRODUCERS.iter().any(|k| t.starts_with(k)) {
            continue;
        }
        // `extern "C" fn name` and `extern crate` both start with `extern `;
        // only the one that reaches an `fn` is a definition.
        let Some(fi) = t.find("fn ") else {
            continue;
        };
        let name: String = t[fi + 3..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        out.push((off + (line.len() - t.len()), name));
    }
    out
}

fn line_offsets(body: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut off = 0usize;
    for line in body.split_inclusive('\n') {
        out.push((off, line.trim_end_matches('\n')));
        off += line.len();
    }
    out
}

/// The code of `chunk`, without its commentary.
///
use anvil::source_scan::without_commentary as code_only;

/// Every function that walks a unified diff itself.
///
/// Test modules are removed first: a fixture that spells a diff is not a parser.
fn hand_rolled_parsers() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for path in rust_sources() {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let body = without_test_modules(&text);
        let rel = posix_rel(&path);

        let starts = function_starts(&body);
        for (idx, (start, name)) in starts.iter().enumerate() {
            let end = starts.get(idx + 1).map(|(s, _)| *s).unwrap_or(body.len());
            let chunk = code_only(&body[*start..end]);
            if DIFF_MARKERS.iter().any(|m| chunk.contains(m)) {
                found.insert(format!("{rel}::{name}"));
            }
        }
    }
    found
}

#[test]
fn hand_rolled_diff_parsing_is_exactly_what_was_recorded() {
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

    assert_eq!(
        counted.len(),
        CEILING,
        "{} function(s) parse a diff by hand; CEILING records {CEILING}.\n\
         If this ROSE: a new one may not be added. Take the files from \
         `diff_context::diffs_by_path`, which reads the path from a header that \
         states it and attributes nothing to a hunk naming no file. Thirteen \
         gates published a path they had not read out of the diff, because \
         thirteen places each parsed one.\n\
         If this FELL: lower CEILING to {} in this same change -- that is what \
         makes it a ratchet rather than a budget.\n  {}",
        counted.len(),
        counted.len(),
        counted
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
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
