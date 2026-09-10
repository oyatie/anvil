//! Two pull requests share one working tree, so only one may write at a time.
//!
//! `StateManager::acquire_pr_lock` serialises a pull request against itself.
//! That is a different question from the one that matters here: two DIFFERENT
//! pull requests hold different PR locks and reach the same clone, because
//! `ensure_repo_cloned` makes exactly one per repository.
//!
//! The fixer checks out `pr-<n>` there, writes model output over minutes, then
//! `add -A` and pushes `HEAD:<head_branch>`. With only a per-PR lock, #2's
//! checkout lands between #1's checkout and #1's commit, and #1 pushes #2's
//! tree onto #1's branch. The head SHA cannot catch it: the fixer takes one and
//! never reads it.

use anvil::git_manager::GitManager;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn manager() -> Arc<GitManager> {
    let dir = tempfile::tempdir().expect("a scratch directory");
    Arc::new(GitManager::new(dir.keep()))
}

/// The property, exercised rather than asserted about: at no instant are two
/// holders inside the critical section for one repository.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_pull_requests_never_hold_one_clone_at_once() {
    let git = manager();
    let inside = Arc::new(AtomicUsize::new(0));
    let overlaps = Arc::new(AtomicUsize::new(0));

    let mut writers = Vec::new();
    for _ in 0..8 {
        let git = git.clone();
        let inside = inside.clone();
        let overlaps = overlaps.clone();
        writers.push(tokio::spawn(async move {
            for _ in 0..25 {
                let lock = git.lock_clone("oyatie/anvil").await;
                let _guard = lock.lock().await;
                if inside.fetch_add(1, Ordering::SeqCst) != 0 {
                    overlaps.fetch_add(1, Ordering::SeqCst);
                }
                // A yield across the critical section: without it the section
                // is short enough that a broken lock could pass by luck.
                tokio::task::yield_now().await;
                inside.fetch_sub(1, Ordering::SeqCst);
            }
        }));
    }
    for writer in writers {
        writer.await.expect("no writer panicked");
    }

    assert_eq!(
        overlaps.load(Ordering::SeqCst),
        0,
        "two writers were inside one repository's clone at the same time, which \
         is the window where one pull request's tree is pushed onto another's \
         branch"
    );
}

/// A cloned manager must not hand out a different lock.
///
/// `GitManager` derives `Clone`, so a per-instance map would give each clone
/// its own locks -- a lock that locks nothing, and one no single-threaded test
/// would notice.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_cloned_manager_shares_the_same_lock() {
    let git = manager();
    let twin = GitManager::clone(&git);

    let first = git.lock_clone("oyatie/anvil").await;
    let held = first.lock().await;

    let second = twin.lock_clone("oyatie/anvil").await;
    assert!(
        second.try_lock().is_err(),
        "a cloned GitManager handed out a different lock for the same clone, so \
         the two would write to one working tree concurrently"
    );
    drop(held);
}

/// Different repositories must not wait on each other.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_repositories_do_not_block_each_other() {
    let git = manager();
    let anvil = git.lock_clone("oyatie/anvil").await;
    let held = anvil.lock().await;

    let other = git.lock_clone("oyatie/oyatie").await;
    assert!(
        other.try_lock().is_ok(),
        "one repository's clone lock blocked another's, which serialises the \
         whole fleet behind whichever repository is slowest"
    );
    drop(held);
}
