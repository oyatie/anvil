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
