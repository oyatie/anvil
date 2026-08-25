use anvil::doc_archival_sweeper::IssueDocConsolidator;
use tempfile::tempdir;

#[tokio::test]
async fn test_issue_doc_consolidator_archives_referenced_plans() {
    let dir = tempdir().unwrap();
    let grok_dir = dir.path().join(".grok/programs");
    tokio::fs::create_dir_all(&grok_dir).await.unwrap();

    let plan_file = grok_dir.join("REORG-REBRAND-BACKLOG.md");
    tokio::fs::write(&plan_file, "# Legacy Reorg Backlog")
        .await
        .unwrap();

    let body = "This issue concludes work in `.grok/programs/REORG-REBRAND-BACKLOG.md`. Ready to consolidate.";
    let report = IssueDocConsolidator::consolidate_issue_docs(dir.path(), 500, body, false)
        .await
        .unwrap();

    assert_eq!(report.files_archived.len(), 1);
    assert_eq!(report.stubs_written.len(), 1);

    let stub = tokio::fs::read_to_string(&plan_file).await.unwrap();
    assert!(stub.contains("HISTORICAL / ARCHIVED (Issue #500)"));
}
