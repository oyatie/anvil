//! Reauthorize fixer entry after waiting for the shared PR lock.

use std::future::Future;
use tokio::sync::Mutex;

pub(super) async fn run(
    state: super::AppState,
    repo: String,
    pr_number: u64,
    head_branch: String,
    head_sha: String,
    is_cross_repository: bool,
    feedback: super::ReviewFeedbackItem,
) {
    // The review pipeline's per-PR lock; both work in one clone.
    let lock = state.state_mgr.acquire_pr_lock(&repo, pr_number).await;
    let _ = after_lock(
        &lock,
        || state.pause.holds(&repo, pr_number, "fixing"),
        || async {
            state
                .fixer
                .resolve_and_fix(
                    &repo,
                    pr_number,
                    &head_branch,
                    &head_sha,
                    is_cross_repository,
                    &[feedback],
                )
                .await
        },
    )
    .await;
}

/// Keep serialization through the work; a held pause prevents even creating it.
/// This is an entry decision, not cancellation of an already running turn.
async fn after_lock<F: Future>(
    lock: &Mutex<()>,
    paused: impl FnOnce() -> bool,
    work: impl FnOnce() -> F,
) -> Option<F::Output> {
    let _guard = lock.lock().await;
    if paused() {
        return None;
    }
    Some(work().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::task::Poll;

    async fn assert_pending(future: std::pin::Pin<&mut impl Future>) {
        let mut future = future;
        std::future::poll_fn(|cx| {
            assert!(future.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
    }

    #[tokio::test]
    async fn pause_engaged_during_lock_wait_prevents_work_creation() {
        let lock = Mutex::new(());
        let guard = lock.lock().await;
        let paused = Cell::new(false);
        let reads = Cell::new(0);
        let calls = Cell::new(0);
        let mut entry = Box::pin(after_lock(
            &lock,
            || {
                reads.set(reads.get() + 1);
                paused.get()
            },
            || {
                calls.set(calls.get() + 1);
                async { 7 }
            },
        ));
        assert_pending(entry.as_mut()).await;
        assert_eq!(reads.get(), 0);
        paused.set(true);
        drop(guard);
        assert_eq!(entry.await, None);
        assert_eq!(reads.get(), 1);
        assert_eq!(calls.get(), 0);
        assert!(lock.try_lock().is_ok());
    }

    #[tokio::test]
    async fn clear_pause_enters_once_and_keeps_the_lock() {
        let lock = Mutex::new(());
        let calls = Cell::new(0);
        let result = after_lock(
            &lock,
            || false,
            || {
                calls.set(calls.get() + 1);
                async {
                    assert!(lock.try_lock().is_err());
                    7
                }
            },
        )
        .await;
        assert_eq!(result, Some(7));
        assert_eq!(calls.get(), 1);
        assert!(lock.try_lock().is_ok());
    }

    #[tokio::test]
    async fn cancelled_lock_wait_never_creates_work() {
        let lock = Mutex::new(());
        let guard = lock.lock().await;
        let calls = Cell::new(0);
        let mut entry = Box::pin(after_lock(
            &lock,
            || false,
            || {
                calls.set(calls.get() + 1);
                async {}
            },
        ));
        assert_pending(entry.as_mut()).await;
        drop(entry);
        drop(guard);
        assert_eq!(calls.get(), 0);
        assert!(lock.try_lock().is_ok());
    }

    #[test]
    fn actual_handler_binds_pause_and_fixer_to_the_locked_entry() {
        let source = crate::source_scan::paths::module_source(
            "src/webhook/webhook_handlers",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
        );
        let normalized = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        let source = normalized(&crate::source_scan::without_commentary(&source));
        let parent = normalized(
            r#"
            fix_entry::run(state_clone, repo_clone, pr_number, head_branch,
                head_sha, is_cross_repository, feedback_item,).await
        "#,
        );
        assert_eq!(source.matches(&parent).count(), 1);
        let expected = normalized(
            r#"
            let lock = state.state_mgr.acquire_pr_lock(&repo, pr_number).await;
            let _ = after_lock(
                &lock,
                || state.pause.holds(&repo, pr_number, "fixing"),
                || async {
                    state.fixer.resolve_and_fix(
                        &repo, pr_number, &head_branch, &head_sha,
                        is_cross_repository, &[feedback],
                    ).await
                },
            ).await
        "#,
        );
        assert_eq!(source.matches(&expected).count(), 1);
        assert_eq!(source.matches(".resolve_and_fix(").count(), 1);
    }
}
