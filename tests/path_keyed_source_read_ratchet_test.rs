//! A test that reads production source by path goes blind the day it is split.
//!
//! Splitting an oversized file into a directory is routine here: the
//! oversized-file budget demands it, and this tree has already done it to
//! `gate_proof`, `pre_merge_guard`, `harness::rules`, `occupancy`,
//! `merge_enlister`, `brand_absence` and `roadmap_guard`. A check that names
//! its subject by file stops finding that subject the day it moves.
//!
//! It stops BLIND, not failing. A read that falls back to an empty string
//! finds nothing wrong in it and publishes a clean verdict on a module nobody
//! read -- absent evidence reported as a pass, the first half of invariant I1,
//! committed inside the checks that exist to enforce it. The reads that instead
//! `expect` are merely loud: they fail for a move that changed nothing they
//! were checking.
//!
//! # The remedy this gate names
//!
//! `source_scan::paths::module_source(module, repo_root)` takes a MODULE. It
//! reads `thing.rs` if that is the form the module takes and every `.rs` under
//! `thing/` if it is a directory instead, strips test modules so a scan of
//! production code cannot be answered by a fixture, and panics for a module
//! that is not there rather than returning an empty string.
//!
//! # What this gate refuses
//!
//! A `read_to_string` or `include_str!` under `tests/` whose argument names a
//! path into the source directory ending in the Rust extension, including
//! where that path is bound to a local one line above the read. It is a ban
//! rather than a count: the sweep that brings this gate leaves one site, and
//! one named exemption is cheaper to read than a ceiling nobody can attribute.
//!
//! # What it does not refuse, stated rather than left to be discovered
//!
//! A helper that reads a path assembled from its own parameter, with the
//! literal spelled at the call site: the read carries no literal and the call
//! site performs no read, so neither half trips this scan. The sweep removed
//! every such helper by giving each one a module for a parameter, and nothing
//! here stops the next one being written. Closing that needs a call graph.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Reads allowed to name a path, with the reason each one is allowed.
///
/// Keyed to the declaration that performs the read, never to a line number.
/// Each entry should name a reason a reviewer can disagree with; an allowlist
/// that grows without argument is the same as no gate.
/// Empty, and that is the whole point: the one exemption that stood here is
/// gone because the read it excused is gone. `line_citations_do_not_grow_against_the_merge_base`
/// named `src/fidelity/registry.rs` on the working-tree side, which was
/// accurate until the registry's entries moved into `registry/entries_*.rs` and
/// the coordinate stopped naming the corpus. It now walks the directory both
/// sides already agreed on, so the exemption has no subject and is removed
/// rather than left to be inherited by whatever takes that name next.
const ALLOWED: &[(&str, &str)] = &[];

/// The calls that read a file.
///
/// `include_str!` is here because it is the same defect with a louder failure:
/// it names a path, it cannot name a directory, and a split turns it into a
/// build break rather than a silent pass.
const READ_CALLS: &[&str] = &["read_to_string", "include_str!"];

/// Every `.rs` file under `tests/` of a repository root.
///
/// Takes the root as an argument so the scan can be pointed at a seeded tree
/// as well as at this one. A gate that can only run against the tree it is
/// committed in cannot be shown to catch anything.
fn test_sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.join("tests")];
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

/// The text between a call's opening parenthesis and its matching close.
fn argument_at(body: &str, open: usize) -> &str {
    let mut depth = 0i32;
    for (k, c) in body[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &body[open + 1..open + k];
                }
            }
            _ => {}
        }
    }
    ""
}

/// Whether `text` carries a string literal naming a path into the source tree.
///
/// The needle is assembled from pieces so that this gate's own source carries
/// no string its own scan would match. A gate that accuses itself is one the
/// first engineer it blocks deletes.
fn names_a_source_path(text: &str) -> bool {
    let prefix = format!("{}{}", "src", '/');
    let suffix = format!("{}{}{}", '.', "rs", '"');
    let mut rest = text;
    while let Some(i) = rest.find(&prefix) {
        let after = &rest[i + prefix.len()..];
        if let Some(end) = after.find('"')
            && after[..end + 1].ends_with(&suffix)
        {
            return true;
        }
        rest = after;
    }
    false
}

/// The value most recently bound to `ident` above the read.
///
/// One hop, not a dataflow analysis. It exists because the commonest spelling
/// of this defect joins the path on one line and reads it on the next, and a
/// scan of the argument alone calls that clean.
fn binding_of(head: &str, ident: &str) -> Option<String> {
    let mut found = None;
    for keyword in ["let ", "const "] {
        let mut base = 0usize;
        while let Some(i) = head[base..].find(keyword) {
            let at = base + i + keyword.len();
            base = at;
            let tail = &head[at..];
            let name: String = tail
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if name != ident {
                continue;
            }
            if let Some(eq) = tail.find('=')
                && let Some(semi) = tail.find(';')
                && eq < semi
            {
                found = Some(tail[eq + 1..semi].to_string());
            }
        }
    }
    found
}

/// The declaration a byte offset sits inside, by the nearest `fn` above it.
fn enclosing_declaration(head: &str) -> String {
    let mut name = "<file scope>".to_string();
    let mut base = 0usize;
    while let Some(i) = head[base..].find("fn ") {
        let at = base + i + 3;
        base = at;
        let candidate: String = head[at..]
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if !candidate.is_empty() {
            name = candidate;
        }
    }
    name
}

/// The bare identifier an argument consists of, when it is only that.
fn sole_identifier(argument: &str) -> Option<&str> {
    let bare = argument.trim().trim_start_matches('&').trim();
    let ok = !bare.is_empty() && bare.chars().all(|c| c.is_alphanumeric() || c == '_');
    ok.then_some(bare)
}

/// Every read under `tests/` that names a path into the source directory.
fn path_keyed_reads(root: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for path in test_sources(root) {
        let body = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        for call in READ_CALLS {
            let mut base = 0usize;
            while let Some(i) = body[base..].find(call) {
                let at = base + i;
                base = at + call.len();
                let Some(open) = body[at..].find('(').map(|o| at + o) else {
                    continue;
                };
                let argument = argument_at(&body, open);
                let mut text = argument.to_string();
                if let Some(ident) = sole_identifier(argument)
                    && let Some(bound) = binding_of(&body[..at], ident)
                {
                    text.push(' ');
                    text.push_str(&bound);
                }
                if names_a_source_path(&text) {
                    found.insert(format!("{stem}::{}", enclosing_declaration(&body[..at])));
                }
            }
        }
    }
    found
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[test]
fn no_test_reads_production_source_by_path() {
    let found = path_keyed_reads(&repo());
    let allowed: BTreeSet<&str> = ALLOWED.iter().map(|(k, _)| *k).collect();
    let offenders: Vec<&String> = found
        .iter()
        .filter(|k| !allowed.contains(k.as_str()))
        .collect();
    assert!(
        offenders.is_empty(),
        "these reads name a path into the source directory:\n  {}\n\
         Take the text from `anvil::source_scan::paths::module_source(module, \
         repo_root)`, which names the MODULE: it reads whichever form that \
         module takes, file or directory, and refuses one that is absent. A \
         path-keyed read returns nothing the day its subject is split, and the \
         check then reports a clean module it never read.",
        offenders
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn every_allowlist_entry_still_reads_by_path() {
    // Coverage, asserted rather than assumed: a scanner that read nothing
    // reports an empty offender set, which is the clean answer. The exemptions
    // are the fixed points that prove this scan can see.
    let found = path_keyed_reads(&repo());
    let stale: Vec<&str> = ALLOWED
        .iter()
        .map(|(k, _)| *k)
        .filter(|k| !found.contains(*k))
        .collect();
    assert!(
        stale.is_empty(),
        "allowlist entries that no longer read by path: {stale:?}\n\
         Remove them, or the exemption outlives the reason for it and the next \
         declaration to take that name inherits it silently."
    );
}

/// The scan must find every spelling of this defect the sweep met.
///
/// Run against a seeded tree rather than argued about, because a scan keyed to
/// one spelling would have reported the sweep complete while the shapes it
/// cannot see stayed behind. The negative case is here for the same reason: a
/// scan that matched every read would be a ban on reading files.
#[test]
fn the_scan_finds_a_path_keyed_read_in_a_seeded_tree() {
    let ext = format!("{}{}", '.', "rs");
    let module = format!("{}{}", "src", "/thing");
    let dir = tempfile::tempdir().expect("temp tree");
    let tests = dir.path().join("tests");
    fs::create_dir_all(&tests).expect("tests dir");
    let seeded = format!(
        "fn direct() {{ fs::read_to_string(\"{module}{ext}\"); }}\n\
         fn bound() {{ let p = root.join(\"{module}{ext}\"); fs::read_to_string(&p); }}\n\
         fn included() {{ let s = include_str!(\"../{module}{ext}\"); }}\n\
         fn innocent() {{ fs::read_to_string(root.join(\"Cargo.toml\")); }}\n"
    );
    fs::write(tests.join("seeded_test.rs"), &seeded).expect("seed");

    let found = path_keyed_reads(dir.path());
    for declaration in [
        "seeded_test::direct",
        "seeded_test::bound",
        "seeded_test::included",
    ] {
        assert!(
            found.contains(declaration),
            "the scan is blind to the read in `{declaration}`; found {found:?}"
        );
    }
    assert!(
        !found.contains("seeded_test::innocent"),
        "a read of something that is not a module was counted as one"
    );
}
