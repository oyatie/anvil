use anvil::dual_track_build_guard::DualTrackBuildGuard;
use anvil::git_manager::PrDiffContext;
use tempfile::tempdir;

#[test]
fn test_dual_track_detects_unreconciled_cargo_change() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\n").unwrap();
    std::fs::write(dir.path().join("BUCK"), "# Buck root\n").unwrap();

    let guard = DualTrackBuildGuard::new();
    let diff_ctx = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 555,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "+ tokio = \"1.38\"".to_string(),
        changed_files: vec!["crates/core/Cargo.toml".to_string()],
        repo_working_dir: dir.path().to_path_buf(),
        is_incremental: false,
        previous_head_sha: None,
    };

    let report = guard
        .evaluate_dual_track_build(dir.path(), &diff_ctx)
        .unwrap();
    assert!(!report.is_synchronized);
    assert!(!report.reindeer_synced);
    assert!(report.summary.contains("FAILED"));
}
