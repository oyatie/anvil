//! A gate on a pre-existing condition is a ratchet, not a threshold.
//!
//! `evaluate_whole_file` read the file from disk and judged its total state,
//! so any change touching a large file inherited its size and any change
//! touching a core file inherited every I/O import already in it. Anvil has 57
//! files over the 300-line budget, which made all 57 unmergeable — including
//! by the decomposition the gate exists to demand.

use anvil::monorepo_guard::whole_file_expansion::{FileChange, WholeFileExpansion};
use std::fs;
use tempfile::tempdir;

fn oversized(dir: &std::path::Path, rel: &str, lines: usize) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, "pub fn f() {}\n".repeat(lines)).unwrap();
}

fn findings(dir: &std::path::Path, rel: &str, old: usize, new: usize) -> Vec<String> {
    let source = fs::read_to_string(dir.join(rel)).unwrap();
    let removed = "-pub fn removed() {}\n".repeat(old);
    let added = source
        .lines()
        .take(new)
        .map(|line| format!("+{line}\n"))
        .collect::<String>();
    let files = anvil::git_manager::diff_context::diffs_by_path(&format!(
        "diff --git a/{rel} b/{rel}\n--- a/{rel}\n+++ b/{rel}\n@@ -1,{old} +1,{new} @@\n{removed}{added}"
    ));
    WholeFileExpansion::evaluate_whole_file(dir, rel, &FileChange::from_diff(&files[0]))
        .expect("evaluate fixture")
        .into_iter()
        .map(|v| v.category)
        .collect()
}

#[test]
fn shrinking_an_oversized_file_is_not_a_finding() {
    // The remedy the gate asks for. Refusing it is the defect that made 57
    // files permanently unmergeable.
    let d = tempdir().unwrap();
    oversized(d.path(), "src/big.rs", 500);
    let f = findings(d.path(), "src/big.rs", 41, 1);
    assert!(
        !f.iter().any(|c| c == "OVERSIZED_WHOLE_FILE"),
        "a change that shrinks the file must pass: {f:?}"
    );
}

#[test]
fn growing_an_already_oversized_file_is_a_finding() {
    let d = tempdir().unwrap();
    oversized(d.path(), "src/big.rs", 500);
    let f = findings(d.path(), "src/big.rs", 1, 13);
    assert!(f.iter().any(|c| c == "OVERSIZED_WHOLE_FILE"), "{f:?}");
}

#[test]
fn touching_an_oversized_file_without_growing_it_is_not_a_finding() {
    // A rename, a comment fix, a one-for-one substitution.
    let d = tempdir().unwrap();
    oversized(d.path(), "src/big.rs", 500);
    let f = findings(d.path(), "src/big.rs", 1, 1);
    assert!(!f.iter().any(|c| c == "OVERSIZED_WHOLE_FILE"), "{f:?}");
}

#[test]
fn a_small_file_grown_past_the_budget_is_a_finding() {
    let d = tempdir().unwrap();
    oversized(d.path(), "src/new.rs", 340);
    let f = findings(d.path(), "src/new.rs", 1, 340);
    assert!(f.iter().any(|c| c == "OVERSIZED_WHOLE_FILE"), "{f:?}");
}

#[test]
fn an_io_import_that_was_already_there_is_not_this_changes_finding() {
    // A typo fix in a core file must not inherit blame for every sqlx:: line
    // already in it.
    let d = tempdir().unwrap();
    let p = d.path().join("src/billing/core/mod.rs");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, "use sqlx::Pool;\npub fn already() {}\n").unwrap();
    // A deletion adds no occurrence of the surviving old import.
    let f = findings(d.path(), "src/billing/core/mod.rs", 1, 0);
    assert!(
        !f.iter()
            .any(|c| c == "CLEAN_ARCHITECTURE_CORE_IO_VIOLATION"),
        "the sqlx line predates this change: {f:?}"
    );
}

#[test]
fn an_io_import_this_change_adds_is_still_caught() {
    let d = tempdir().unwrap();
    let p = d.path().join("src/billing/core/mod.rs");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, "use sqlx::Pool;\npub fn f() {}\n").unwrap();
    let f = findings(d.path(), "src/billing/core/mod.rs", 1, 1);
    assert!(
        f.iter()
            .any(|c| c == "CLEAN_ARCHITECTURE_CORE_IO_VIOLATION"),
        "the change added it, so it is the change's finding: {f:?}"
    );
}
