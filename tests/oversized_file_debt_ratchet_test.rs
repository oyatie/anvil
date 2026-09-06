//! The debt the diff-scoped gate no longer charges to whoever touches it.
//!
//! `evaluate_whole_file` now charges the change: a file is a finding only if
//! this change grew it past the budget. That removes a false attribution — 57
//! files here are over ADR-0719 D-35's 300-line budget, and blaming every
//! toucher made all 57 unmergeable, including by the split the rule demands.
//!
//! Removing the attribution must not remove the debt. Diff-scoped enforcement
//! and a whole-repo baseline that may only shrink are two mechanisms, and a
//! codebase adopting a rule it already violates needs both: Google generates a
//! baseline when a lint lands in legacy code and enforces no-new-violations
//! plus a burn-down; GitHub Advanced Security splits alerts into new (blocks)
//! and pre-existing (tracked); Meta lints the diff and carries the backlog
//! separately. This is that second mechanism.

use std::fs;
use std::path::{Path, PathBuf};

/// ADR-0719 D-35. Not a policy this test may relax.
const BUDGET: usize = 300;

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            rust_sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// Files over budget, and by how much in total.
fn debt(root: &Path) -> (usize, usize) {
    let mut files = Vec::new();
    rust_sources(root, &mut files);
    let mut count = 0;
    let mut excess = 0;
    for p in files {
        let Ok(body) = fs::read_to_string(&p) else {
            continue;
        };
        let n = body.lines().count();
        if n > BUDGET {
            count += 1;
            excess += n - BUDGET;
        }
    }
    (count, excess)
}

fn src_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

#[test]
fn the_oversized_file_debt_does_not_grow_against_the_merge_base() {
    // Derived, not declared. A committed count is a global every lane must
    // edit, and two branches that both lower it write the same line and merge
    // cleanly — leaving a number that is wrong with no conflict to catch it.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rt = tokio::runtime::Runtime::new().expect("runtime");
    let derived = rt.block_on(anvil::ratchet::facade::derived::at_merge_base(
        repo,
        "origin/dev",
        "HEAD",
        |p| p.starts_with("src/") && p.ends_with(".rs"),
        |tree| {
            let mut count = 0usize;
            let mut excess = 0usize;
            for path in tree.paths() {
                if !(path.starts_with("src/") && path.ends_with(".rs")) {
                    continue;
                }
                let Ok(Some(bytes)) = tree.read(path) else {
                    continue;
                };
                let Ok(text) = std::str::from_utf8(bytes) else {
                    continue;
                };
                let n = text.lines().count();
                if n > BUDGET {
                    count += 1;
                    excess += n - BUDGET;
                }
            }
            (count, excess)
        },
    ));
    let Ok(base) = derived else {
        eprintln!("skipped: no merge-base against origin/dev in this checkout");
        return;
    };
    let (count, excess) = debt(&src_root());
    let (base_count, base_excess) = base.at_merge_base;
    assert!(
        count <= base_count,
        "files over the {BUDGET}-line budget grew from {base_count} at merge-base \
         {} to {count}. The gate no longer charges whoever touches an oversized \
         file, so this is what keeps the debt visible.",
        &base.merge_base[..12]
    );
    assert!(
        excess <= base_excess,
        "total lines over budget grew from {base_excess} to {excess}. A file may \
         be split, moved or shrunk; it may not be fattened."
    );
}

#[test]
fn the_budget_itself_is_not_relaxed_by_this_test() {
    // A ratchet whose threshold can move is not a ratchet. D-35 is 300.
    assert_eq!(
        BUDGET, 300,
        "ADR-0719 D-35 fixes the hand-written file budget"
    );
    assert_eq!(
        BUDGET,
        anvil::monorepo_guard::whole_file_expansion::WholeFileExpansion::MAX_WHOLE_FILE_LINES,
        "the debt ratchet and the per-change gate must measure the same budget"
    );
}
