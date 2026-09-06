//! The FinOps gate must distinguish "scanned and clean" from "nothing to scan".

use anvil::finops_ratchet::FinOpsUnitCostRatchet;
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::GateStatus;
use std::path::{Path, PathBuf};

fn ctx(diff: &str) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "main".to_string(),
        base_sha: "a".to_string(),
        head_sha: "b".to_string(),
        diff_content: diff.to_string(),
        changed_files: vec![],
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            PathBuf::from("."),
            anvil::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    }
}

/// The scanner only inspects files whose path contains one of `network/`,
/// `codec/`, `engine/`, `hotpath` or `packet`. None of those markers matches
/// a single tracked file in this repository, so nothing is ever in scope and
/// `findings.is_empty()` is vacuously true -- the gate reported "zero
/// unbudgeted heap allocations in hotpaths" for a hotpath set it never had.
///
/// An empty scope is not a clean scan.
#[test]
fn a_diff_with_no_hotpath_files_is_not_measured() {
    let rep = FinOpsUnitCostRatchet::new()
        .evaluate_unit_cost(
            Path::new("."),
            &ctx("diff --git a/src/lib.rs b/src/lib.rs\n+ let v = vec![1];"),
        )
        .expect("evaluates");

    assert_eq!(
        rep.status.unmeasured_gate_id(),
        Some("finops_status"),
        "no file was in the hotpath scope, so nothing was measured"
    );
    assert!(!rep.is_cost_optimal);
}

/// A real hotpath file must still be scanned and judged, or the gate would
/// pass its first test by never measuring anything at all.
#[test]
fn a_hotpath_file_is_still_scanned_and_judged() {
    let clean = FinOpsUnitCostRatchet::new()
        .evaluate_unit_cost(
            Path::new("."),
            &ctx("diff --git a/src/network/mod.rs b/src/network/mod.rs\n+ let n = 1;"),
        )
        .expect("evaluates");

    assert!(
        matches!(clean.status, GateStatus::Passed),
        "a hotpath file with no avoidable allocation must pass: {:?}",
        clean.status
    );
}
