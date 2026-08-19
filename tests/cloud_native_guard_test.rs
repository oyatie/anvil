use anvil::cloud_native_guard::CloudNativeGuard;
use anvil::git_manager::PrDiffContext;
use std::path::Path;

#[test]
fn test_cloud_native_guard_violations() {
    let guard = CloudNativeGuard::new();

    // 1. Violation: Hardcoded ARN
    let diff_ctx1 = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1001,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: r#"+ let role = "arn:aws:iam::123456789012:role/MyRole";"#.to_string(),
        changed_files: vec!["crates/iam/src/lib.rs".to_string()],
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        is_incremental: false,
        previous_head_sha: None,
    };
    let r1 = guard
        .evaluate_cloud_native(Path::new("/tmp"), &diff_ctx1)
        .unwrap();
    assert!(!r1.is_compliant);
    assert!(r1
        .violations
        .iter()
        .any(|v| v.category == "HARDCODED_CLOUD_ENDPOINT"));

    // 2. Violation: New Python script in scripts/
    let diff_ctx2 = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1002,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "+ print('hello')".to_string(),
        changed_files: vec!["scripts/deploy.py".to_string()],
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        is_incremental: false,
        previous_head_sha: None,
    };
    let r2 = guard
        .evaluate_cloud_native(Path::new("/tmp"), &diff_ctx2)
        .unwrap();
    assert!(!r2.is_compliant);
    assert!(r2
        .violations
        .iter()
        .any(|v| v.category == "NON_RUST_SCRIPT_TOOLING"));

    // 3. Clean: Provider-agnostic trait port
    let diff_ctx3 = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1003,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "+ pub trait ObjectStore { async fn get(&self) -> Vec<u8>; }".to_string(),
        changed_files: vec!["storage/ports/src/lib.rs".to_string()],
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        is_incremental: false,
        previous_head_sha: None,
    };
    let r3 = guard
        .evaluate_cloud_native(Path::new("/tmp"), &diff_ctx3)
        .unwrap();
    assert!(r3.is_compliant);
}
