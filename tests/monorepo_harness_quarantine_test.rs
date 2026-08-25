use anvil::monorepo_guard::HarnessQuarantine;

#[test]
fn test_blocks_ai_harness_commit_leaks() {
    let dirty_changed = vec![
        ".grok/programs/SPRINT_PLAN.md".to_string(),
        ".claude/scratch.json".to_string(),
        ".antigravity/state.bin".to_string(),
        ".codex/instructions.md".to_string(),
        "crates/core/src/lib.rs".to_string(),
    ];

    let violations = HarnessQuarantine::check_harness_quarantine(&dirty_changed);
    assert_eq!(violations.len(), 4);
    for v in &violations {
        assert_eq!(v.category, "AI_HARNESS_COMMIT_LEAK");
    }

    let clean_changed = vec![
        "crates/core/src/lib.rs".to_string(),
        "docs/adr/ADR-0701.md".to_string(),
    ];
    let clean_violations = HarnessQuarantine::check_harness_quarantine(&clean_changed);
    assert!(clean_violations.is_empty());
}

#[test]
fn test_enforces_ssot_location_boundary() {
    let content_authority = "---\ncanonical_authority: true\n---\n# Rule";

    // Non-canonical location fails
    let fail1 =
        HarnessQuarantine::check_ssot_authority_location("tenancy/standard.md", content_authority);
    assert!(fail1.is_some());
    assert_eq!(fail1.unwrap().category, "UNAUTHORIZED_AUTHORITY_CLAIM");

    let fail2 =
        HarnessQuarantine::check_ssot_authority_location("iac/k8s/guide.md", content_authority);
    assert!(fail2.is_some());

    // Approved canonical locations pass
    let pass_docs = HarnessQuarantine::check_ssot_authority_location(
        "docs/decisions/ADR-0701.md",
        content_authority,
    );
    assert!(pass_docs.is_none());

    let pass_contracts = HarnessQuarantine::check_ssot_authority_location(
        "contracts/openapi/api.yaml",
        content_authority,
    );
    assert!(pass_contracts.is_none());
}
