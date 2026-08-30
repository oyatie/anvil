use anvil::state::{PrState, StateManager, WalEntry};
use tempfile::tempdir;

#[tokio::test]
async fn test_state_manager_wal_persistence_and_atomic_checkpoint() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let state_mgr = StateManager::load(tmp.path())
        .await
        .expect("Failed to load StateManager");

    // 1. Mutate PR state
    let state1 = state_mgr
        .update_pr_state(
            "oyatie/anvil",
            10,
            "sha-alpha-123".to_string(),
            Some("APPROVED".to_string()),
        )
        .await
        .expect("update failed");

    assert_eq!(state1.last_reviewed_head_sha, "sha-alpha-123");
    assert_eq!(state1.review_count, 1);

    // 2. Verify state retrieval
    let retrieved = state_mgr
        .get_pr_state("oyatie/anvil", 10)
        .await
        .expect("state not found");
    assert_eq!(retrieved.last_reviewed_head_sha, "sha-alpha-123");
    assert_eq!(retrieved.last_review_verdict.as_deref(), Some("APPROVED"));

    // 3. Record certification
    state_mgr
        .record_certification("oyatie/anvil", 10, "sha-alpha-123", true)
        .await
        .expect("certification failed");

    let certified = state_mgr
        .get_pr_state("oyatie/anvil", 10)
        .await
        .expect("certified state not found");
    assert_eq!(
        certified.last_certified_head_sha.as_deref(),
        Some("sha-alpha-123")
    );
    assert!(certified.is_enlisted_in_merge_queue);

    // 4. Simulate crash and restart: Reload from disk and verify durability
    let reloaded = StateManager::load(tmp.path())
        .await
        .expect("Failed to reload StateManager");
    let state_after_restart = reloaded
        .get_pr_state("oyatie/anvil", 10)
        .await
        .expect("state not found after restart");
    assert_eq!(state_after_restart.last_reviewed_head_sha, "sha-alpha-123");
    assert_eq!(
        state_after_restart.last_certified_head_sha.as_deref(),
        Some("sha-alpha-123")
    );
    assert!(state_after_restart.is_enlisted_in_merge_queue);
}

#[tokio::test]
async fn test_state_manager_wal_crash_recovery_replay() {
    let tmp = tempdir().expect("Failed to create tempdir");
    let wal_path = tmp.path().join("pr_states.wal");

    // Synthesize an uncheckpointed WAL log simulating sudden power loss before atomic rename
    let uncheckpointed_entry = WalEntry {
        timestamp: "2026-08-19T20:45:00Z".to_string(),
        key: "oyatie/oyatie#2159".to_string(),
        state: PrState {
            auto_fix_attempts: 0,
            last_auto_fixed_head_sha: None,
            last_reviewed_head_sha: "uncheckpointed-sha-999".to_string(),
            last_reviewed_at: "2026-08-19T20:45:00Z".to_string(),
            review_count: 5,
            last_review_verdict: Some("APPROVED".to_string()),
            last_certified_head_sha: Some("uncheckpointed-sha-999".to_string()),
            is_enlisted_in_merge_queue: true,
        },
    };

    let serialized = serde_json::to_string(&uncheckpointed_entry).unwrap();
    tokio::fs::write(&wal_path, format!("{}\n", serialized))
        .await
        .unwrap();

    // Boot StateManager and assert that the uncheckpointed mutation was replayed
    let state_mgr = StateManager::load(tmp.path())
        .await
        .expect("Failed to load StateManager");
    let recovered = state_mgr
        .get_pr_state("oyatie/oyatie", 2159)
        .await
        .expect("WAL recovery failed");

    assert_eq!(recovered.last_reviewed_head_sha, "uncheckpointed-sha-999");
    assert_eq!(recovered.review_count, 5);
    assert!(recovered.is_enlisted_in_merge_queue);
}

/// A rollback that only the checkpoint records is a rollback a crash loses.
///
/// `update_pr_state` writes the WAL before the checkpoint, because the
/// checkpoint is a whole-file rewrite and the log is append-only: a crash
/// between them is recovered from the log. `clear_reviewed_sha` wrote no log
/// entry at all, so an interrupted or failed checkpoint silently lost the
/// rollback -- and the pull request came back from the restart stamped at a
/// head it had already been reviewed for, which is the stranding the rollback
/// exists to abolish, arriving by the one path nothing watched.
///
/// The checkpoint deletes the log on success, so the only way to observe the
/// entry is to make the checkpoint fail. The directory is made unwritable,
/// which stops the checkpoint's temp file while leaving the already-created
/// log appendable.
#[cfg(unix)]
#[tokio::test]
async fn the_reviewed_sha_rollback_reaches_the_write_ahead_log() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().expect("tempdir");
    let sm = StateManager::load(tmp.path())
        .await
        .expect("a fresh state manager");

    sm.update_pr_state("oyatie/anvil", 7, "cafe1234".to_string(), None)
        .await
        .expect("stamped");

    // The log the rollback must reach. `update_pr_state` removed it when its
    // checkpoint succeeded; recreate it so the append below has a writable
    // file once the directory itself is not.
    let wal_path = tmp.path().join("pr_states.wal");
    tokio::fs::write(&wal_path, "")
        .await
        .expect("log recreated");

    let dir_perms = std::fs::metadata(tmp.path()).expect("stat").permissions();
    std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(0o555))
        .expect("make the directory unwritable");

    sm.clear_reviewed_sha("oyatie/anvil", 7).await;

    let wal = tokio::fs::read_to_string(&wal_path)
        .await
        .unwrap_or_default();
    std::fs::set_permissions(tmp.path(), dir_perms).expect("restore");

    let cleared = wal
        .lines()
        .filter(|l| l.contains("oyatie/anvil#7"))
        .filter(|l| {
            serde_json::from_str::<serde_json::Value>(l)
                .ok()
                .and_then(|v| {
                    v.get("state")?
                        .get("last_reviewed_head_sha")?
                        .as_str()
                        .map(|s| s.is_empty())
                })
                .unwrap_or(false)
        })
        .count();

    assert!(
        cleared >= 1,
        "the rollback never reached the log, so a checkpoint that does not \
         land leaves this pull request stamped at a head nothing will review \
         again. The log holds:\n{wal}"
    );
}
