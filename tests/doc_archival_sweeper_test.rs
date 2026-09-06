use anvil::doc_archival_sweeper::DocArchivalSweeper;
use tempfile::tempdir;

#[tokio::test]
async fn test_sweeps_stale_grok_plans_and_writes_stubs() {
    let dir = tempdir().unwrap();
    let grok_dir = dir.path().join(".grok/programs");
    tokio::fs::create_dir_all(&grok_dir).await.unwrap();

    let plan_file = grok_dir.join("REORG.md");
    tokio::fs::write(&plan_file, "# Temporary Reorg Plan")
        .await
        .unwrap();

    let report = DocArchivalSweeper::sweep_repository(dir.path(), false)
        .await
        .unwrap();
    assert_eq!(report.files_archived.len(), 1);
    assert_eq!(report.stubs_written.len(), 1);

    let stub_content = tokio::fs::read_to_string(&plan_file).await.unwrap();
    assert!(stub_content.contains("status: archived"));
    assert!(stub_content.contains("archive/2026/.grok/programs/REORG.md"));
}
