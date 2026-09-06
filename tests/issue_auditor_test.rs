use anvil::issue_reconciler::{IssueAuditStatus, IssueAuditor};
use tempfile::tempdir;

#[test]
fn test_issue_auditor_detects_contradiction_and_recovery() {
    let dir = tempdir().unwrap();
    let decisions_dir = dir.path().join("docs/decisions");
    std::fs::create_dir_all(&decisions_dir).unwrap();

    // 1. Contradicted by ADR
    let f1 = IssueAuditor::audit_issue(
        dir.path(),
        101,
        "Storage layout SeaweedFS primary",
        "Based on ADR-0196 mandates",
    );
    assert_eq!(f1.status, IssueAuditStatus::ContradictedByADR);
    assert!(f1.resolution_receipt.is_some());

    // 2. Resolved by trunk CI recovery
    let f2 = IssueAuditor::audit_issue(
        dir.path(),
        102,
        "🚨 Trunk CI Failure in build lane",
        "Failed cargo check",
    );
    assert_eq!(f2.status, IssueAuditStatus::ResolvedByCommit);
    assert!(f2.resolution_receipt.is_some());

    // 3. Active valid issue
    let f3 = IssueAuditor::audit_issue(
        dir.path(),
        103,
        "Add Prometheus metric for RPC latency",
        "Implement histogram in telemetry module",
    );
    assert_eq!(f3.status, IssueAuditStatus::Active);
}
