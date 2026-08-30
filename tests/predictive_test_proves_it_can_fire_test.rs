//! The predictive test selector demonstrates both halves, and its third answer.
//!
//! The gate measures SELECTION, not wall-clock: nothing in it times a test run,
//! so "optimized" can only mean the selection is a strict subset of the
//! workspace. `is_optimized` used to be the literal `true`, which is why the
//! spared half matters as much as the fired one.

use anvil::git_manager::{PrDiffContext, SubjectRoot, Uncloned};
use anvil::pre_merge_guard::report::GateStatus;

fn ctx(files: Vec<&str>) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: String::new(),
        changed_files: files.into_iter().map(str::to_string).collect(),
        repo_working_dir: SubjectRoot::asserted(
            std::path::PathBuf::from("."),
            Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    }
}

/// A two-member cargo workspace on disk.
fn workspace(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("anvil-dag-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for member in ["alpha", "beta"] {
        std::fs::create_dir_all(root.join(member).join("src")).expect("scratch");
        std::fs::write(
            root.join(member).join("Cargo.toml"),
            format!("[package]\nname = \"{member}\"\nversion = \"0.1.0\"\n"),
        )
        .expect("member manifest");
        std::fs::write(root.join(member).join("src/lib.rs"), "").expect("member source");
    }
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = [\"alpha\", \"beta\"]\n",
    )
    .expect("workspace manifest");
    root
}

#[test]
fn predictive_test_spares_a_change_confined_to_one_package() {
    let root = workspace("green");
    let report = anvil::predictive_test_selector::PredictiveTestSelector::new()
        .evaluate_test_selection(&root, &ctx(vec!["alpha/src/lib.rs"]))
        .expect("the selector runs");
    assert!(
        report.is_optimized,
        "a change touching one member of a two-member workspace can spare the \
         other, and the selector reported no pruning: {}",
        report.summary
    );
    assert!(report.skipped_packages_count > 0);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn predictive_test_fires_when_nothing_can_be_spared() {
    let root = workspace("red");
    let report = anvil::predictive_test_selector::PredictiveTestSelector::new()
        .evaluate_test_selection(&root, &ctx(vec!["alpha/src/lib.rs", "beta/src/lib.rs"]))
        .expect("the selector runs");
    assert!(
        !report.is_optimized,
        "every member was touched, so nothing was pruned. Reporting `optimized` \
         here is the literal `true` this gate used to publish: {}",
        report.summary
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// The answer that is neither: no workspace was discovered.
#[test]
fn predictive_test_withholds_when_no_workspace_was_discovered() {
    let root = std::env::temp_dir().join(format!("anvil-dag-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("scratch");
    let report = anvil::predictive_test_selector::PredictiveTestSelector::new()
        .evaluate_test_selection(&root, &ctx(vec!["alpha/src/lib.rs"]))
        .expect("the selector runs");
    assert!(
        matches!(report.status, GateStatus::NotMeasured { .. }),
        "an undiscovered workspace is not a one-package workspace. Discovery \
         used to fall back to a hand-written package so the gate always had \
         something to report: {:?}",
        report.status
    );
    let _ = std::fs::remove_dir_all(&root);
}
