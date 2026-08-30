use super::*;
#[test]
fn test_catches_hallucinated_redis_and_apex_adr_mutation() {
    let guard = StackWhitelistGuard::new();
    let diff_ctx = PrDiffContext {
        repo: "oyatie/oyatie".to_string(),
        pr_number: 999,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        diff_content: "+ use redis::Client;".to_string(),
        changed_files: vec![
            "docs/decisions/ADR-0701-monorepo-capability-live-apex.md".to_string(),
            "crates/cache/src/lib.rs".to_string(),
        ],
        repo_working_dir: crate::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("/tmp"),
            crate::git_manager::Uncloned::TestFixture,
        ),
        is_incremental: false,
        previous_head_sha: None,
    };

    let report = guard
        .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, false)
        .unwrap();
    assert!(!report.is_compliant);
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.category == "APEX_ADR_IMMUTABILITY_BREACH")
    );
    assert!(
        report
            .violations
            .iter()
            .any(|v| v.category == "UNAPPROVED_STACK_TECHNOLOGY")
    );
}
