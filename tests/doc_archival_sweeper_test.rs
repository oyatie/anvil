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
/// The archive destination is blocked by a FILE where a directory is needed,
/// so `create_dir_all` fails and the copy cannot be written.
///
/// # The precondition is asserted, and that is the point
///
/// This test asserted only that the original was intact and no stub recorded.
/// Both are true when the sweeper never looks at the file at all: review seeded
/// the trigger `.grok/programs/` to `.grokx/programs/` and this test stayed
/// green while its two siblings went red. A test that passes when the subject
/// does nothing proves nothing about the subject.
///
/// `archive_failed` is what makes the precondition assertable. It could not be
/// `files_archived`, because the fix for the false-report defect is that
/// `files_archived` stays EMPTY here -- so proving the file was considered and
/// proving it was not archived needed two fields rather than one.
#[tokio::test]
async fn a_failed_archive_write_leaves_the_original_alone() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    const ORIGINAL: &str = "# Plan\n\nStill the only copy.\n";

    let grok_dir = root.join(".grok/programs");
    tokio::fs::create_dir_all(&grok_dir).await.unwrap();
    let plan = grok_dir.join("REORG.md");
    tokio::fs::write(&plan, ORIGINAL).await.unwrap();

    tokio::fs::create_dir_all(root.join("archive"))
        .await
        .unwrap();
    tokio::fs::write(root.join("archive/2026"), "in the way\n")
        .await
        .unwrap();

    let report = DocArchivalSweeper::sweep_repository(root, false)
        .await
        .unwrap();

    // PRECONDITION: the sweeper reached this file and tried.
    assert_eq!(
        report.archive_failed,
        vec![".grok/programs/REORG.md".to_string()],
        "the file must have been considered for archival, or everything below \
         is true of a sweeper that did nothing"
    );

    assert_eq!(
        tokio::fs::read_to_string(&plan).await.unwrap(),
        ORIGINAL,
        "the archive copy could not be written, so the original must still be here"
    );
    assert!(
        report.files_archived.is_empty(),
        "nothing was archived, and the report may not say otherwise: {:?}",
        report.files_archived
    );
    assert!(report.stubs_written.is_empty());
}

/// Sweeping twice must not destroy what sweeping once saved.
///
/// After one sweep the original is a stub and the archive holds the content.
/// The original path still matches the trigger, so on the next run the sweeper
/// read the stub as `content` and truncate-overwrote the only surviving copy
/// with it: sweep 1 saved the document, sweep 2 destroyed it.
///
/// `anvil doc-sweep --repo X` is a repeatable command over a repository whose
/// sweep output is meant to be committed, so a second sweep is the expected
/// case and not an edge one.
#[tokio::test]
async fn a_second_sweep_does_not_overwrite_what_the_first_archived() {
    let dir = tempdir().unwrap();
    let root = dir.path();
    const ORIGINAL: &str = "# Plan\n\nThe only copy of this text.\n";

    let grok_dir = root.join(".grok/programs");
    tokio::fs::create_dir_all(&grok_dir).await.unwrap();
    tokio::fs::write(grok_dir.join("REORG.md"), ORIGINAL)
        .await
        .unwrap();

    let first = DocArchivalSweeper::sweep_repository(root, false)
        .await
        .unwrap();
    assert_eq!(first.stubs_written.len(), 1, "the first sweep must archive");

    let second = DocArchivalSweeper::sweep_repository(root, false)
        .await
        .unwrap();

    let archived = root.join("archive/2026/.grok/programs/REORG.md");
    assert_eq!(
        tokio::fs::read_to_string(&archived).await.unwrap(),
        ORIGINAL,
        "a second sweep overwrote the only surviving copy with the stub"
    );
    assert!(
        second.files_archived.is_empty(),
        "an already-archived file must not be archived again: {:?}",
        second.files_archived
    );
}

/// The archive must not still claim the authority the sweep just removed.
///
/// A `.grok/programs/*.md` holding `canonical_authority: true` matches BOTH
/// branches. The demotion rewrote the original and the archival branch then
/// wrote the PRE-demotion text to the archive, so the report said the claim was
/// demoted while the only surviving copy still asserted it -- and the next
/// sweep demoted the archive itself, writing into `archive/2026/` and reporting
/// the same document twice.
#[tokio::test]
async fn the_archive_holds_the_demoted_text_not_the_claim_that_was_removed() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    let grok_dir = root.join(".grok/programs");
    tokio::fs::create_dir_all(&grok_dir).await.unwrap();
    tokio::fs::write(
        grok_dir.join("REORG.md"),
        "---\ncanonical_authority: true\n---\n# Plan\n",
    )
    .await
    .unwrap();

    let report = DocArchivalSweeper::sweep_repository(root, false)
        .await
        .unwrap();
    assert_eq!(report.ssot_claims_demoted.len(), 1);

    let archived = tokio::fs::read_to_string(root.join("archive/2026/.grok/programs/REORG.md"))
        .await
        .unwrap();
    assert!(
        archived.contains("canonical_authority: false"),
        "the report said this claim was demoted; the surviving copy still \
         asserts it: {archived:?}"
    );
    assert!(
        archived.contains("# Plan"),
        "and it must still be the document"
    );
}
