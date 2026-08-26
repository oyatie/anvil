//! Unit tests for the guard: same crate, so private items stay reachable.
//!
//! Kept under the 300-line budget alongside the modules they exercise
//! (ADR-0719 D-35 does not exempt tests).

use super::*;

#[test]
fn test_catches_core_importing_adapter() {
    let guard = CleanArchitectureGuard::new();
    let diff_ctx = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 201,
        base_branch: "main".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        diff_content: "+++ b/repos/oyatie/tenancy/core/src/tenant.rs\n+ use crate::adapters::postgres::PgPool;".to_string(),
        changed_files: vec!["repos/oyatie/tenancy/core/src/tenant.rs".to_string()],
        is_incremental: false,
    };

    let report = guard.evaluate_architecture(&diff_ctx).expect("Evaluates");
    assert!(!report.is_clean);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].source_layer, "CORE/DOMAIN");
}

#[test]
fn layer_tokens_match_whole_words_not_substrings() {
    // Observed on Anvil's own tree: `pub use report::Finding;` in a core
    // module matched `ports?` through "re|port", and `rest` would match
    // "forest". A layer name is a path segment, not a substring.
    let guard = CleanArchitectureGuard::new();
    let clean = PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 204,
        base_branch: "main".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        diff_content: "+++ b/src/shape/core/mod.rs\n+ pub use report::Finding;\n+ use crate::forest::Tree;\n+ use crate::supporting::Thing;".to_string(),
        changed_files: vec!["src/shape/core/mod.rs".to_string()],
        is_incremental: false,
    };
    let report = guard.evaluate_architecture(&clean).expect("Evaluates");
    assert!(
        report.is_clean,
        "substring hits must not be violations: {:?}",
        report.violations
    );
    assert!(report.measurement.is_measured());

    let dirty = PrDiffContext {
        diff_content: "+++ b/src/shape/core/mod.rs\n+ use crate::ports::TreeSource;".to_string(),
        ..clean
    };
    let report = guard.evaluate_architecture(&dirty).expect("Evaluates");
    assert_eq!(
        report.violations.len(),
        1,
        "a real ports import must still fire"
    );
    assert_eq!(report.violations[0].target_layer, "PORTS/APPLICATION");
}

#[test]
fn comments_and_strings_are_not_dependency_edges() {
    let guard = CleanArchitectureGuard::new();
    let ctx = PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 205,
        base_branch: "main".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        diff_content: "+++ b/src/shape/core/dependency.rs\n+ // denied target (facade -> ports, because ports -> core)\n+ /// an adapters face is where adapters live\n+ let msg = \"use the ports face\";\n+ use super::tree::TreeSource;".to_string(),
        changed_files: vec!["src/shape/core/dependency.rs".to_string()],
        is_incremental: false,
    };
    let report = guard.evaluate_architecture(&ctx).expect("Evaluates");
    assert!(report.is_clean, "{:?}", report.violations);
    assert!(report.measurement.is_measured());
}

#[test]
fn test_valid_inward_adapter_import_passes() {
    let guard = CleanArchitectureGuard::new();
    let diff_ctx = PrDiffContext {
        repo: "oyatie/console".to_string(),
        pr_number: 202,
        base_branch: "main".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        diff_content: "+++ b/repos/console/backend/crates/payroll/adapter-postgres/src/repo.rs\n+ use payroll_domain::PayrollRecord;\n+ use payroll_ports::PayrollStore;".to_string(),
        changed_files: vec!["repos/console/backend/crates/payroll/adapter-postgres/src/repo.rs".to_string()],
        is_incremental: false,
    };

    let report = guard.evaluate_architecture(&diff_ctx).expect("Evaluates");
    assert!(report.is_clean);
    assert!(report.measurement.is_measured());
}

#[test]
fn test_unlayered_diff_is_not_measured_rather_than_clean() {
    let guard = CleanArchitectureGuard::new();
    let diff_ctx = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 203,
        base_branch: "main".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        diff_content: "+++ b/src/util.rs\n+ use crate::adapters::pg::Pool;".to_string(),
        changed_files: vec!["src/util.rs".to_string()],
        is_incremental: false,
    };

    let report = guard.evaluate_architecture(&diff_ctx).expect("Evaluates");
    assert!(!report.is_clean);
    assert!(report.violations.is_empty());
    assert!(report.measurement.not_measured_reason().is_some());
    assert!(report.summary.to_lowercase().contains("not measured"));
}

#[test]
fn test_missing_source_tree_is_not_measured() {
    let guard = CleanArchitectureGuard::new();
    let report = guard
        .evaluate_source_tree(Path::new("/nonexistent/anvil/src"))
        .expect("Evaluates");
    assert!(!report.is_clean);
    assert!(report.measurement.not_measured_reason().is_some());
}

#[test]
fn test_self_conformance_reads_anvils_own_tree() {
    let guard = CleanArchitectureGuard::new();
    let report = guard.self_conformance().expect("Evaluates");
    // Deliberately asserts nothing about Anvil being clean: only that the
    // guard actually read the tree and reported a state it can defend.
    assert!(
        report.measurement.files_inspected() > 0,
        "self-conformance read no files from {ANVIL_SOURCE_TREE}"
    );
    if report.measurement.files_classified() == 0 {
        assert!(!report.is_clean);
        assert!(report.summary.to_lowercase().contains("not measured"));
    }
}
