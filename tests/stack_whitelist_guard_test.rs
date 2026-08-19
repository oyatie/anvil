use anvil::git_manager::PrDiffContext;
use anvil::stack_whitelist_guard::StackWhitelistGuard;
use std::path::Path;

#[test]
fn test_stack_whitelist_guard_catches_unapproved_mongodb_and_actix() {
    let guard = StackWhitelistGuard::new();

    let diff_ctx = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1200,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "+ use mongodb::Client;\n+ use actix_web::App;".to_string(),
        changed_files: vec!["crates/api/src/lib.rs".to_string()],
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        is_incremental: false,
        previous_head_sha: None,
    };

    let report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, false)
        .unwrap();
    assert!(!report.is_compliant);
    assert_eq!(report.violations.len(), 2);
    assert!(report.violations.iter().any(|v| v.item == "mongodb::"));
    assert!(report.violations.iter().any(|v| v.item == "actix_web"));
}

#[test]
fn test_apex_adr_immutability_lock_allows_human_blocks_agent() {
    let guard = StackWhitelistGuard::new();

    let diff_ctx = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1201,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "+ Modified apex doctrine".to_string(),
        changed_files: vec!["docs/decisions/ADR-0709-general-live-apex.md".to_string()],
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        is_incremental: false,
        previous_head_sha: None,
    };

    // Agent PR fails
    let agent_report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, false)
        .unwrap();
    assert!(!agent_report.is_compliant);
    assert!(agent_report
        .violations
        .iter()
        .any(|v| v.category == "APEX_ADR_IMMUTABILITY_BREACH"));

    // Human PR passes
    let human_report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, true)
        .unwrap();
    assert!(human_report.is_compliant);
}

#[test]
fn test_asymmetric_dependency_ratchet_allows_removal_blocks_addition() {
    let guard = StackWhitelistGuard::new();

    // 1. Agent adding a dependency fails
    let add_diff = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1202,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "[dependencies]\n+ serde_yaml = \"0.9\"".to_string(),
        changed_files: vec!["crates/api/Cargo.toml".to_string()],
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        is_incremental: false,
        previous_head_sha: None,
    };
    let add_report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &add_diff, false)
        .unwrap();
    assert!(!add_report.is_compliant);
    assert!(add_report
        .violations
        .iter()
        .any(|v| v.category == "UNAUTHORIZED_DEPENDENCY_EXPANSION"));

    // 2. Agent removing an unused/misplaced dependency passes cleanly!
    let remove_diff = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 1203,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "[dependencies]\n- legacy_crate = \"0.1\"".to_string(),
        changed_files: vec!["crates/api/Cargo.toml".to_string()],
        repo_working_dir: std::path::PathBuf::from("/tmp"),
        is_incremental: false,
        previous_head_sha: None,
    };
    let remove_report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &remove_diff, false)
        .unwrap();
    assert!(remove_report.is_compliant);
}
