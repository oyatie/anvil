//! Ordinary state transitions: no daemon, process interruption, or Git fixture.
use anvil::recovery::needs_certification;
use anvil::state::StateManager;
use anvil::webhook::pipelines::admit::{Admission, admit};

const REPO: &str = "owner/repo";
const HEAD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

async fn completed(store: &StateManager) {
    store
        .update_pr_state(REPO, 1, HEAD.into(), Some("COMMENT".into()))
        .await
        .unwrap();
    store
        .record_pipeline_completion(REPO, 1, HEAD)
        .await
        .unwrap();
}

#[tokio::test]
async fn same_head_new_stamp_invalidates_previous_completion_durably() {
    let dir = tempfile::tempdir().unwrap();
    let store = StateManager::load(dir.path()).await.unwrap();
    completed(&store).await;
    let prior = store.get_pr_state(REPO, 1).await.unwrap();
    assert_eq!(admit(true, Some(&prior), HEAD), Admission::Forced);

    store
        .update_pr_state(REPO, 1, HEAD.into(), Some("APPROVE".into()))
        .await
        .unwrap();
    // Omit completion deliberately: this models persisted unfinished state,
    // not a real crash or any process-control operation.
    let reloaded = StateManager::load(dir.path()).await.unwrap();
    let unfinished = reloaded.get_pr_state(REPO, 1).await.unwrap();
    assert!(needs_certification(Some(&unfinished), HEAD));
    assert_eq!(unfinished.last_completed_head_sha, None);
    assert_eq!(admit(false, Some(&unfinished), HEAD), Admission::Recovering);
    assert_eq!(unfinished.review_count, 2);
    assert_eq!(unfinished.last_review_verdict.as_deref(), Some("APPROVE"));
}

#[tokio::test]
async fn finishing_the_second_attempt_restores_deliberate_skip() {
    let dir = tempfile::tempdir().unwrap();
    let store = StateManager::load(dir.path()).await.unwrap();
    completed(&store).await;
    store
        .update_pr_state(REPO, 1, HEAD.into(), Some("COMMENT".into()))
        .await
        .unwrap();
    store
        .record_pipeline_completion(REPO, 1, HEAD)
        .await
        .unwrap();
    let reloaded = StateManager::load(dir.path()).await.unwrap();
    let finished = reloaded.get_pr_state(REPO, 1).await.unwrap();
    assert!(needs_certification(Some(&finished), HEAD));
    assert_eq!(admit(false, Some(&finished), HEAD), Admission::Skip);
}

#[tokio::test]
async fn untouched_finished_head_is_not_recovered() {
    let dir = tempfile::tempdir().unwrap();
    let store = StateManager::load(dir.path()).await.unwrap();
    completed(&store).await;
    let finished = store.get_pr_state(REPO, 1).await.unwrap();
    assert_eq!(admit(false, Some(&finished), HEAD), Admission::Skip);
    assert_eq!(finished.review_count, 1);
}
