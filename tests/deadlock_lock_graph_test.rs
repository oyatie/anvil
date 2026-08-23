//! The `deadlock_status` gate: a lock-order graph with cycle detection.
//!
//! # Defect this file exists to prevent
//!
//! The gate previously scanned for four literal identifiers -- `session_lock`,
//! `user_mutex`, `global_state`, `cluster_mutex` -- none of which occurred
//! anywhere in this repository outside the analyzer's own test fixture. Its
//! finding rate on real code was structurally zero, so `passed` was a constant
//! wearing the costume of a measurement. A gate that cannot fire is not a gate.
//!
//! # Why the tests live here and not in a `#[cfg(test)]` module
//!
//! [`no_finding_on_this_repository_s_own_source_tree`] runs the scanner over
//! every `.rs` file under `src/`. A firing fixture parked inside the scanner's
//! own source would be found by that scan, and the only way to keep the scan
//! green would be to exclude the file -- which is exactly the exclusion that let
//! the previous implementation's only evidence be its own fixture. Keeping the
//! fixtures in `tests/` means the real-tree scan has no exclusions at all.
//!
//! # The two directions
//!
//! A gate can be dishonest twice. It can never fire (the defect above), or it
//! can fire on everything, in which case it blocks every pull request and gets
//! switched off. Every test below is paired: a firing case that fails if the
//! scanner does nothing, and a clean case that fails if the scanner fires
//! indiscriminately. Neither a `vec![]` nor an unconditional finding survives
//! this file.

use anvil::deadlock_analyzer::{DeadlockStaticAnalyzer, LockOrderGraph};
use std::path::{Path, PathBuf};

fn scan(code: &str) -> Vec<anvil::deadlock_analyzer::LockOrderFinding> {
    LockOrderGraph::new().find_lock_order_cycles("src/fixture.rs", code)
}

// ---------------------------------------------------------------------------
// 1. Inverted acquisition order across two sites -- the classic 2-cycle
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// The scanner returning `vec![]` unconditionally, or recognising only a
/// hardcoded list of lock names. Neither `accounts` nor `ledger` is special;
/// the cycle is what makes this a finding.
#[test]
fn fires_when_the_same_pair_is_acquired_in_both_orders() {
    let code = r#"
impl Bank {
    fn credit(&self) {
        let a = self.accounts.lock();
        let l = self.ledger.lock();
        drain(a, l);
    }
    fn audit(&self) {
        let l = self.ledger.lock();
        let a = self.accounts.lock();
        drain(a, l);
    }
}
"#;
    let findings = scan(code);
    assert_eq!(
        findings.len(),
        1,
        "one cycle over {{accounts, ledger}} expected, got {findings:?}"
    );
    let seq = &findings[0].lock_sequence;
    assert!(
        seq.iter().any(|l| l == "self.accounts") && seq.iter().any(|l| l == "self.ledger"),
        "the finding names both locks in the cycle: {seq:?}"
    );
    assert_eq!(findings[0].file_path, "src/fixture.rs");
}

/// # Defect this catches
///
/// A scanner that reports any two distinct locks held together. Holding two
/// locks is not a bug; holding them in *inconsistent* orders is. Without this
/// test the implementation could pass test 1 by flagging every nesting, and
/// every pull request that touches `account_pool/manager.rs` would be blocked.
#[test]
fn silent_when_every_site_acquires_the_pair_in_the_same_order() {
    let code = r#"
impl Bank {
    fn credit(&self) {
        let a = self.accounts.lock();
        let l = self.ledger.lock();
        drain(a, l);
    }
    fn debit(&self) {
        let a = self.accounts.lock();
        let l = self.ledger.lock();
        drain(a, l);
    }
}
"#;
    assert!(
        scan(code).is_empty(),
        "consistent lock order is not a deadlock"
    );
}

// ---------------------------------------------------------------------------
// 2. Cycles longer than two -- proves this is a graph search, not a pair check
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// Special-casing the 2-cycle with `edges.contains(&(b, a))`. That passes tests
/// 1 and 2 while missing every A->B->C->A deadlock, which is the shape real
/// lock hierarchies fail in once more than two locks exist.
#[test]
fn fires_on_a_three_lock_cycle_that_no_pairwise_check_would_find() {
    let code = r#"
fn one(&self) {
    let a = self.alpha.lock();
    let b = self.beta.lock();
    go(a, b);
}
fn two(&self) {
    let b = self.beta.lock();
    let c = self.gamma.lock();
    go(b, c);
}
fn three(&self) {
    let c = self.gamma.lock();
    let a = self.alpha.lock();
    go(c, a);
}
"#;
    let findings = scan(code);
    assert_eq!(findings.len(), 1, "one 3-cycle expected, got {findings:?}");
    assert_eq!(
        findings[0].lock_sequence.len(),
        3,
        "the cycle names all three locks: {:?}",
        findings[0].lock_sequence
    );
}

/// A three-lock hierarchy acquired in one consistent total order is the correct
/// way to use three locks. It must not be reported.
#[test]
fn silent_on_a_three_lock_hierarchy_with_a_consistent_total_order() {
    let code = r#"
fn one(&self) {
    let a = self.alpha.lock();
    let b = self.beta.lock();
    go(a, b);
}
fn two(&self) {
    let b = self.beta.lock();
    let c = self.gamma.lock();
    go(b, c);
}
fn three(&self) {
    let a = self.alpha.lock();
    let c = self.gamma.lock();
    go(a, c);
}
"#;
    assert!(scan(code).is_empty(), "alpha < beta < gamma is a hierarchy");
}

// ---------------------------------------------------------------------------
// 3. Self-deadlock: re-acquiring one non-reentrant lock while holding it
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// Dropping self-edges from the graph. `Mutex` and `RwLock` in `std`, `tokio`
/// and `parking_lot` are all non-reentrant: taking the same lock twice on one
/// task deadlocks it outright. It is a 1-cycle in the same graph and needs no
/// second site to be a bug.
#[test]
fn fires_when_one_lock_is_taken_twice_while_still_held() {
    let code = r#"
fn refresh(&self) {
    let first = self.cache.write();
    let second = self.cache.read();
    use_both(first, second);
}
"#;
    let findings = scan(code);
    assert_eq!(
        findings.len(),
        1,
        "self-deadlock expected, got {findings:?}"
    );
    assert_eq!(findings[0].lock_sequence, vec!["self.cache".to_string()]);
}

/// # Defect this catches
///
/// Ignoring guard scope, which is the single largest false-positive source.
/// `state.rs::acquire_pr_lock` really does read-lock `self.locks` and then
/// write-lock it -- but the read guard is bound inside a block that closes
/// first, so the write is taken with nothing held. A scanner that tracks lock
/// names without tracking the braces around them reports this real, correct,
/// shipped code as a deadlock.
#[test]
fn silent_when_the_guard_s_block_closed_before_the_second_acquisition() {
    let code = r#"
fn acquire(&self) {
    {
        let read = self.locks.read();
        if let Some(l) = read.get(&key) {
            return l.clone();
        }
    }
    let mut write = self.locks.write();
    write.insert(key, value);
}
"#;
    assert!(
        scan(code).is_empty(),
        "a guard dropped at the end of its block holds nothing afterwards"
    );
}

// ---------------------------------------------------------------------------
// 4. Temporaries are not held
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// Treating every `.lock()` as a held guard. A guard that is not bound to a
/// `let` is a temporary and is dropped at the end of its statement, so it is
/// never held across the next acquisition. `ratchet/adapters/git_merge_base.rs`
/// ships exactly this shape (`self.cache.lock().map_err(..)?.insert(..)`);
/// counting it as held manufactures edges out of correct code.
#[test]
fn silent_when_the_outer_acquisition_is_a_statement_temporary() {
    let code = r#"
fn one(&self) {
    let l = self.left.lock();
    let r = self.right.lock();
    go(l, r);
}
fn two(&self) {
    self.right.lock().clear();
    let l = self.left.lock();
    go(l);
}
"#;
    assert!(
        scan(code).is_empty(),
        "a temporary guard is dropped at the end of its statement, so it opens no edge"
    );
}

/// The mirror: a temporary acquired *while* a bound guard is held is a real
/// nested acquisition and must still produce an edge, so the exclusion above
/// cannot be implemented by ignoring unbound acquisitions altogether.
#[test]
fn fires_when_a_temporary_is_acquired_inside_a_bound_guard_s_scope() {
    let code = r#"
fn one(&self) {
    let l = self.left.lock();
    self.right.lock().clear();
}
fn two(&self) {
    let r = self.right.lock();
    self.left.lock().clear();
}
"#;
    let findings = scan(code);
    assert_eq!(
        findings.len(),
        1,
        "an unbound inner acquisition still nests: {findings:?}"
    );
}

/// # Defect this catches
///
/// Naming a node the scanner cannot actually name. `shard(i).lock()` and
/// `queues[0].lock()` acquire *some* lock, but which one is a function of a
/// runtime value, and a scanner that falls back to the empty string -- or to the
/// text before the call -- collapses every such acquisition onto one node. Two
/// of them at either end of a nesting then close a cycle that exists nowhere in
/// the program. `ratchet/adapters/git_merge_base.rs` writes `.lock()` on its own
/// continuation line, which is the same shape.
#[test]
fn silent_when_the_receiver_is_a_call_or_index_result_that_cannot_be_named() {
    let code = r#"
fn one(&self) {
    let s = shard(index).lock();
    let l = self.ledger.lock();
    go(s, l);
}
fn two(&self) {
    let l = self.ledger.lock();
    let q = queues[0].lock();
    go(l, q);
}
"#;
    assert!(
        scan(code).is_empty(),
        "an unnameable receiver must contribute no node, not a shared one"
    );
}

// ---------------------------------------------------------------------------
// 5. The input is a unified diff, not a file
// ---------------------------------------------------------------------------

/// # Defect this catches
///
/// Scanning raw diff text as if it were Rust. The gate is handed
/// `diff_ctx.diff_content`, so every line carries a `+`, `-` or space marker and
/// the hunks are separated by `@@` headers. A deleted inversion is a *fix*; a
/// scanner that reads `-` lines accuses the author of the bug they just removed.
#[test]
fn removed_lines_are_not_evidence_of_a_lock_order_the_change_deletes() {
    let diff = "diff --git a/src/bank.rs b/src/bank.rs\n\
                @@ -1,8 +1,8 @@\n\
                 fn credit(&self) {\n\
                     let a = self.accounts.lock();\n\
                     let l = self.ledger.lock();\n\
                 }\n\
                 fn audit(&self) {\n\
                -    let l = self.ledger.lock();\n\
                -    let a = self.accounts.lock();\n\
                +    let a = self.accounts.lock();\n\
                +    let l = self.ledger.lock();\n\
                 }\n";
    assert!(
        scan(diff).is_empty(),
        "the inversion exists only on removed lines -- this diff repairs it"
    );
}

/// The same diff with the inversion on the *added* side must fire, so the test
/// above cannot be satisfied by refusing to read diffs at all.
#[test]
fn added_lines_carrying_an_inversion_do_fire() {
    let diff = "diff --git a/src/bank.rs b/src/bank.rs\n\
                @@ -1,8 +1,8 @@\n\
                 fn credit(&self) {\n\
                     let a = self.accounts.lock();\n\
                     let l = self.ledger.lock();\n\
                 }\n\
                 fn audit(&self) {\n\
                +    let l = self.ledger.lock();\n\
                +    let a = self.accounts.lock();\n\
                 }\n";
    assert!(
        !scan(diff).is_empty(),
        "an inversion introduced by this change is exactly what the gate is for"
    );
}

/// # Defect this catches
///
/// Letting held guards leak across a file or hunk boundary. Two hunks are
/// disjoint fragments of unrelated code; a guard opened at the end of one is
/// not held at the start of the next, and pairing across the seam invents a
/// nesting that exists in no function.
#[test]
fn a_guard_does_not_stay_held_across_a_hunk_boundary() {
    let diff = "diff --git a/src/one.rs b/src/one.rs\n\
                @@ -1,2 +1,2 @@\n\
                +    let a = self.accounts.lock();\n\
                +    let l = self.ledger.lock();\n\
                diff --git a/src/two.rs b/src/two.rs\n\
                @@ -1,2 +1,2 @@\n\
                +    let l = self.ledger.lock();\n\
                @@ -9,2 +9,2 @@\n\
                +    let a = self.accounts.lock();\n";
    assert!(
        scan(diff).is_empty(),
        "the reverse edge would only exist if a guard survived a hunk header"
    );
}

// ---------------------------------------------------------------------------
// 6. The report the evaluator reads
// ---------------------------------------------------------------------------

#[test]
fn the_report_fails_on_a_cycle_and_passes_on_clean_code() {
    let analyzer = DeadlockStaticAnalyzer::new();

    let inverted = "+ fn a() {\n\
                    +     let x = self.alpha.lock();\n\
                    +     let y = self.beta.lock();\n\
                    + }\n\
                    + fn b() {\n\
                    +     let y = self.beta.lock();\n\
                    +     let x = self.alpha.lock();\n\
                    + }\n";
    let report = analyzer.evaluate_deadlock_invariants("oyatie/anvil", inverted);
    assert!(!report.passed, "a lock-order cycle must fail the gate");
    assert_eq!(report.findings.len(), 1);
    assert!(
        report.findings[0].description.contains("self.alpha"),
        "the message names the locks, not a generic accusation: {}",
        report.findings[0].description
    );

    let clean = analyzer.evaluate_deadlock_invariants("oyatie/anvil", "let x = 1;\n");
    assert!(clean.passed);
    assert!(clean.findings.is_empty());
}

// ---------------------------------------------------------------------------
// 7. The false-positive proof: this repository's own source
// ---------------------------------------------------------------------------

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

/// # Defect this catches
///
/// The objection any staff engineer raises to generalising a matcher: it now
/// fires, and it fires on us. `src/` contains real nested locking --
/// `account_pool/manager.rs` holds `self.pools` while write-locking each
/// account, `state.rs` reads and then writes `self.locks` -- and a scanner
/// tuned only against its own fixtures would block every pull request in the
/// repository from the day it merged.
///
/// This is the whole tree, with no exclusions, including the scanner's own
/// source. If a future change makes the analysis looser, this test is what
/// notices, and it notices before the gate is pointed at anybody's pull
/// request.
#[test]
fn no_finding_on_this_repository_s_own_source_tree() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rs(&root.join("src"), &mut files);
    files.sort();
    assert!(files.len() > 100, "the tree walk found nothing to scan");

    let graph = LockOrderGraph::new();
    let mut failures = Vec::new();
    for path in &files {
        let body = std::fs::read_to_string(path).expect("source file is readable");
        for f in graph.find_lock_order_cycles(&path.display().to_string(), &body) {
            failures.push(format!("{}: {}", path.display(), f.description));
        }
    }

    assert!(
        failures.is_empty(),
        "{} false positive(s) on this repository's own correct code:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

/// The scan above is only evidence if the scanner would have found something
/// had something been there. Injecting one inverted pair into the real tree's
/// text must be caught, which proves the green above is a measurement and not
/// an empty walk.
#[test]
fn the_real_tree_scan_would_catch_an_inversion_injected_into_it() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let body = std::fs::read_to_string(root.join("src/state.rs")).expect("state.rs is readable");
    let seeded = format!(
        "{body}\nfn seeded_one() {{\n    let a = self.alpha.lock();\n    let b = self.beta.lock();\n}}\n\
         fn seeded_two() {{\n    let b = self.beta.lock();\n    let a = self.alpha.lock();\n}}\n"
    );
    assert!(
        !LockOrderGraph::new()
            .find_lock_order_cycles("src/state.rs", &seeded)
            .is_empty(),
        "the real-tree scan is blind, not clean"
    );
}
