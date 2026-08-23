//! Gates must not report a pass they have no evidence for.

use anvil::pre_merge_guard::GateStatus;

/// `evaluate_quarantine_lifecycle` set `let passed = true;` as a literal, so
/// `flake_quarantine_status` was `Passed` on every pull request ever certified.
/// Underneath, "quarantine" was a substring match for "flaky" against the
/// changed *file paths* (the parameter is named `modified_tests`), and nothing
/// was ever isolated because there is no quarantine lane to isolate into.
///
/// Anvil retains no test-run history, so it cannot know which tests are flaky.
/// That is the honest answer, and it is not a pass.
#[test]
fn flake_quarantine_reports_no_history_rather_than_a_clean_lane() {
    let report = anvil::flake_quarantine::FlakeQuarantineLifecycle::new()
        .evaluate_quarantine_lifecycle(&["tests/flaky_network_test.rs".to_string()]);

    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some("flake_quarantine_status"),
        "no run history exists, so flakiness is unknown"
    );
    assert!(
        !report.passed,
        "a lane nothing was isolated into is not a clean one"
    );
    assert!(
        !matches!(report.status, GateStatus::Passed),
        "the gate must not stamp a green it cannot justify"
    );
}

/// `is_optimized` was a literal `true`, so `predictive_test_status` passed
/// regardless of what the DAG selection returned -- and when package discovery
/// found nothing, the selector invented a package named "anvil" to select.
#[test]
fn predictive_selection_is_measured_not_asserted() {
    use anvil::predictive_test_selector::PredictiveTestSelector;

    // No workspace to discover under an empty directory: nothing to prune, so
    // nothing to claim.
    let dir = tempfile::tempdir().expect("tempdir");
    let report = PredictiveTestSelector::new()
        .evaluate_test_selection(dir.path(), &diff_ctx(&["src/lib.rs".to_string()]))
        .expect("evaluates");

    assert_eq!(
        report.status.unmeasured_gate_id(),
        Some("predictive_test_status"),
        "no workspace was discovered, so no pruning was measured"
    );
    assert!(!report.is_optimized);
}

fn diff_ctx(changed: &[String]) -> anvil::git_manager::PrDiffContext {
    anvil::git_manager::PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "main".to_string(),
        base_sha: "a".to_string(),
        head_sha: "b".to_string(),
        diff_content: String::new(),
        changed_files: changed.to_vec(),
        repo_working_dir: std::path::PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

/// The measured path must also be measured. Without this, restoring
/// `let is_optimized = true;` kills nothing: the empty-workspace test above
/// covers only the NotMeasured branch, so a constant verdict on the branch
/// that actually runs would go unnoticed. That mutant survived until this
/// test existed.
///
/// A change that touches every package prunes nothing, and pruning nothing is
/// not an optimised selection.
#[test]
fn a_selection_that_prunes_nothing_is_not_reported_as_optimised() {
    use anvil::predictive_test_selector::PredictiveTestSelector;

    let dir = tempfile::tempdir().expect("tempdir");
    // A real single-package workspace: the change touches it, so nothing is
    // spared and there is no pruning to claim.
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"solo\"\nversion = \"0.1.0\"\n",
    )
    .expect("manifest");
    std::fs::create_dir_all(dir.path().join("src")).expect("src");
    std::fs::write(dir.path().join("src/lib.rs"), "pub fn f() {}\n").expect("lib");

    let report = PredictiveTestSelector::new()
        // Selection matches a changed path against the package name, so this
        // path affects the only package there is.
        .evaluate_test_selection(dir.path(), &diff_ctx(&["solo/src/lib.rs".to_string()]))
        .expect("evaluates");

    assert_eq!(
        report.skipped_packages_count, 0,
        "the only package was affected, so nothing was spared"
    );
    assert!(
        !report.is_optimized,
        "pruning nothing is not an optimised selection; this is the assertion \
         a constant `true` would violate"
    );
}
