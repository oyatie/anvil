//! Fixtures for the feature-flag ratchet.

use super::*;

use super::*;

fn diff_ctx(diff: &str) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/console".to_string(),
        pr_number: 301,
        base_branch: "main".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "bbb".to_string(),
        previous_head_sha: None,
        repo_working_dir: crate::git_manager::SubjectRoot::asserted(
            std::path::PathBuf::from("/tmp"),
            crate::git_manager::Uncloned::TestFixture,
        ),
        diff_content: diff.to_string(),
        changed_files: vec!["src/features.ts".to_string()],
        is_incremental: false,
    }
}

#[test]
fn test_flag_usage_without_a_ledger_is_not_a_pass() {
    let temp_dir = tempfile::tempdir().expect("tempdir");
    let report = FeatureFlagRatchet::new()
        .evaluate_feature_flags(
            temp_dir.path(),
            &diff_ctx(
                "+++ b/src/features.ts\n+ if (is_feature_enabled('new_billing_v2')) { doNew(); }",
            ),
        )
        .expect("eval");

    assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
    assert_eq!(report.flags_scanned_count, 1);
    assert!(!report.is_clean);
}

#[test]
fn test_stale_ledger_entry_is_reported_against_the_reference() {
    let report = FeatureFlagRatchet::new().evaluate_flag_lifecycle(
        &[FlagReference {
            file_path: "src/features.ts".to_string(),
            flag_key: "new_billing_v2".to_string(),
        }],
        "- `new_billing_v2`\n",
    );

    assert!(!report.is_clean);
    assert_eq!(report.violations[0].issue_type, "STALE_FLAG_REFERENCED");
}

#[test]
fn test_a_key_mentioned_in_prose_is_not_a_ledger_record() {
    let report = FeatureFlagRatchet::new().evaluate_flag_lifecycle(
        &[FlagReference {
            file_path: "src/features.ts".to_string(),
            flag_key: "new_billing".to_string(),
        }],
        "We are keeping new_billing for now; `new_billing_v2` is stale.\n",
    );

    assert!(
        report.is_clean,
        "`new_billing` is neither backticked nor the whole of the backticked key"
    );
}
