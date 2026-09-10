//! Exclusive use of a repository's shared clone working tree.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// The map [`super::GitManager`] carries.
///
/// `Arc` because that type is `Clone`: a cloned manager with its own map would
/// hand out different locks for the same clone, which is a lock that locks
/// nothing. Sharing the map is what makes the guarantee survive the derive.
pub(super) type CloneLocks = Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>;

/// One repository's clone lock. Lock it to enter; hold the guard to stay in.
pub type CloneLock = Arc<Mutex<()>>;

impl super::GitManager {
    /// Exclusive use of a repository's shared clone WORKING TREE.
    ///
    /// `ensure_repo_cloned` makes one clone per repository and every consumer
    /// shares it. `StateManager::acquire_pr_lock` serialises a pull request
    /// against itself, which is a different question: two DIFFERENT pull
    /// requests hold different PR locks and reach the same working tree.
    ///
    /// The fixer checks out `pr-<n>` there, writes model output over minutes,
    /// then `add -A` and pushes `HEAD:<head_branch>`. Without this, #2's
    /// checkout can land between #1's checkout and #1's commit, and #1 pushes
    /// #2's tree onto #1's branch. The head SHA cannot catch it -- the fixer
    /// takes one and never reads it (`_head_sha`).
    ///
    /// The lock lives here because [`super::GitManager`] owns the clone. A
    /// caller cannot hold it correctly without knowing that, and a caller that
    /// forgets is the defect.
    ///
    /// HOLD IT ACROSS THE WHOLE MUTATION: from before the checkout until after
    /// the push. Releasing it earlier -- after the checkout, say -- leaves the
    /// model turn and the commit unprotected, and that is the window that
    /// matters, because it is minutes long.
    ///
    /// LOCK ORDER: pull request, then clone. Every caller already holds its PR
    /// lock before reaching mutation, so taking these in the other order
    /// anywhere would deadlock against them.
    pub async fn lock_clone(&self, repo: &str) -> CloneLock {
        let key = repo.to_lowercase();
        if let Some(lock) = self.clone_locks.read().await.get(&key) {
            return lock.clone();
        }
        self.clone_locks
            .write()
            .await
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}
