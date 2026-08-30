//! Fixtures for the brand-absence ledger.
//!
//! Its own file, and declared by the parent as `#[cfg(test)] mod tests;`, so
//! that `subject_root_escape_hatches_are_named_test` can tell this is test
//! code. Its brace-depth scan cannot classify a file holding raw strings, and
//! it refuses to guess rather than reading a fixture as a production caller.

use super::*;

/// Regenerates the ledger body. Run with:
/// `cargo test -p anvil brand_absence::tests::print_ledger -- --ignored --nocapture`
#[test]
#[ignore = "generator: prints the KNOWN_VIOLATIONS body for this file"]
fn print_ledger() {
    let gate = BrandAbsenceGate::with_allowlist(Vec::new());
    let report = gate.scan_tree(&crate::git_manager::SubjectRoot::asserted(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        crate::git_manager::Uncloned::TestFixture,
    ));
    let mut counts: std::collections::BTreeMap<(String, String, BrandViolationKind), usize> =
        std::collections::BTreeMap::new();
    for v in &report.new_violations {
        *counts
            .entry((v.path.clone(), v.stamp.clone(), v.kind))
            .or_insert(0) += 1;
    }
    let mut merged: std::collections::BTreeMap<(String, String), (usize, Vec<String>)> =
        std::collections::BTreeMap::new();
    for ((path, stamp, kind), n) in counts {
        let e = merged.entry((path, stamp)).or_insert((0, Vec::new()));
        e.0 += n;
        let label = match kind {
            BrandViolationKind::Name => "name",
            BrandViolationKind::DisplayString => "display string",
            BrandViolationKind::GateCountClaim => "gate-count claim",
        };
        if !e.1.iter().any(|l| l == label) {
            e.1.push(label.to_string());
        }
    }
    for ((path, stamp), (n, kinds)) in &merged {
        let body = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .unwrap_or_default()
            .to_lowercase();
        let stamp = [
            stamp.clone(),
            stamp.replace(' ', "_"),
            stamp.replace(' ', "-"),
            stamp.replace(' ', ""),
        ]
        .into_iter()
        .find(|candidate| body.contains(candidate))
        .unwrap_or_else(|| stamp.clone());
        println!(
            "    AllowlistedDebt {{ path: {path:?}, stamp: {stamp:?}, occurrences: {n}, debt_note: \"pre-existing {}; rename deferred to the retain/discard determination (plan 36.2)\" }},",
            kinds.join(" + ")
        );
    }
    println!("// entries: {}", merged.len());
    println!("{}", report.summary);
}

/// Prints the gate's current verdict over `src/`. Not asserted: the tree is
/// being edited by other lanes, and this gate is warn-only by design, so a
/// new violation must show up in the report rather than break the build.
#[test]
#[ignore = "reporter: prints the warn-only verdict for src/"]
fn print_tree_status() {
    let report = BrandAbsenceGate::new().scan_tree(&crate::git_manager::SubjectRoot::asserted(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        crate::git_manager::Uncloned::TestFixture,
    ));
    for v in &report.new_violations {
        println!(
            "{}:{} {:?} [{}] {}",
            v.path, v.line, v.kind, v.stamp, v.snippet
        );
    }
    println!("{}", report.summary);
}

#[test]
fn real_gate_count_reads_the_corpus() {
    // Pinned to the corpus constant rather than a literal. This test
    // previously hardcoded 68, which is exactly how seven PR-visible
    // strings came to claim 70 against a corpus of 68.
    assert_eq!(
        BrandAbsenceGate::new().real_gate_count(),
        crate::pre_merge_guard::report::TOTAL_GATES
    );
}

#[test]
fn ledger_has_no_duplicate_keys() {
    let mut keys: Vec<(String, String)> =
        load_allowlist(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .iter()
            .map(|e| (e.path.clone(), e.stamp.clone()))
            .collect();
    let before = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(before, keys.len());
}
