use anvil::corpus_auditor::FreshnessLedger;
use tempfile::tempdir;

#[test]
fn test_freshness_ledger_metrics() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("active.rs"), "fn a() {}").unwrap();
    std::fs::write(dir.path().join("active.md"), "# Active").unwrap();

    let report = FreshnessLedger::scan_repository(dir.path(), 180);
    assert_eq!(report.total_files, 2);
    assert_eq!(report.dormant_files_count, 0);
    assert_eq!(report.freshness_ratio, 1.0);
}
