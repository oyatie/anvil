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

/// The archive must hold the content the stub says it holds.
///
/// Issue #253. `dest` was computed, its parent directory created, and nothing
/// ever written to it. The original was then overwritten with a stub reading
/// "Moved to `archive/2026/{rel}`" -- naming a file that did not exist. In a
/// non-dry-run sweep of a contributor's repository the content was destroyed
/// and the forward pointer dangled.
///
/// The test above could not catch it: it asserts the stub CONTAINS the
/// destination path and never asserts the destination is there. A check that
/// the note says the right thing, while the thing it names was never written,
/// is a proxy standing in for the property.
///
/// So this reads the archive back and compares it to what was there before.
#[tokio::test]
async fn the_archived_copy_exists_and_holds_the_original_content() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    const ORIGINAL: &str = "# Temporary Reorg Plan\n\nThe only copy of this text.\n";

    let grok_dir = root.join(".grok/programs");
    tokio::fs::create_dir_all(&grok_dir).await.unwrap();
    let plan = grok_dir.join("REORG.md");
    tokio::fs::write(&plan, ORIGINAL).await.unwrap();

    let report = DocArchivalSweeper::sweep_repository(root, false)
        .await
        .unwrap();
    assert_eq!(report.stubs_written.len(), 1);

    let archived = root.join("archive/2026/.grok/programs/REORG.md");
    assert!(
        archived.exists(),
        "the stub points at {} and nothing wrote it, so the original content is \
         gone and the pointer dangles",
        archived.display()
    );
    assert_eq!(
        tokio::fs::read_to_string(&archived).await.unwrap(),
        ORIGINAL,
        "the archive must hold what the original held, not a second stub"
    );

    // And the original really was replaced, so the assertion above is about a
    // COPY that survived rather than a file that was never moved.
    let left_behind = tokio::fs::read_to_string(&plan).await.unwrap();
    assert!(left_behind.contains("status: archived"));
    assert!(!left_behind.contains("The only copy of this text."));
}

/// A stub may only ever replace content that is already safe somewhere else.
///
/// Every write on this path was `let _ =`, so ordering alone is not enough: a
/// failed archive copy followed by a successful stub write is the same data
/// loss with a healthy-looking report. Here the archive destination is blocked
/// by a FILE where the sweeper needs a directory, so `create_dir_all` fails.
/// The original must survive intact and no stub may be recorded.
#[tokio::test]
async fn a_failed_archive_write_leaves_the_original_alone() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    const ORIGINAL: &str = "# Plan\n\nStill the only copy.\n";

    let grok_dir = root.join(".grok/programs");
    tokio::fs::create_dir_all(&grok_dir).await.unwrap();
    let plan = grok_dir.join("REORG.md");
    tokio::fs::write(&plan, ORIGINAL).await.unwrap();

    // `archive/2026` is a regular file, so creating the destination's parent
    // directory beneath it cannot succeed.
    tokio::fs::create_dir_all(root.join("archive"))
        .await
        .unwrap();
    tokio::fs::write(root.join("archive/2026"), "in the way\n")
        .await
        .unwrap();

    let report = DocArchivalSweeper::sweep_repository(root, false)
        .await
        .unwrap();

    assert_eq!(
        tokio::fs::read_to_string(&plan).await.unwrap(),
        ORIGINAL,
        "the archive copy could not be written, so the original must still be here"
    );
    assert!(
        report.stubs_written.is_empty(),
        "a stub was reported for a file that was never archived: {:?}",
        report.stubs_written
    );
}
