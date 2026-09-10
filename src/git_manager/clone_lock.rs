//! Exclusive use of a repository's shared clone working tree.

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};

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
    /// LOCK ORDER: pull request, then clone, wherever both are held. Not every
    /// caller holds a PR lock -- `reconcile_pr` reaches mutation from the CLI
    /// and the manual webhook handlers without one -- so the rule is about
    /// order, never about a PR lock being present.
    ///
    /// CEILING: this is an in-process mutex over a filesystem resource. A
    /// second Anvil process on the same clone -- the CLI subcommands behind
    /// `PrSelfHealer` and `DocArchivalSweeper` are exactly that -- shares the
    /// working tree and not the map, so it is not excluded. An advisory lock
    /// on the clone directory is the upgrade if that ever bites.
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

/// A repository's clone, and exclusive use of it, inseparably.
///
/// The two-step spelling -- `ensure_repo_cloned` for the path, `lock_clone`
/// for the lock -- lets a caller take the path and skip the lock, and that
/// omission is invisible: the code compiles, the tests pass, and the damage
/// only appears when two pull requests overlap in production. Handing back a
/// value that owns the guard removes the unlocked spelling instead of asking
/// callers to remember.
///
/// The guard is released when this is dropped, so hold it for the whole
/// mutation -- see [`super::GitManager::lock_clone`] for why that matters.
pub struct LockedClone {
    root: super::SubjectRoot,
    _guard: OwnedMutexGuard<()>,
}

impl LockedClone {
    pub fn root(&self) -> &super::SubjectRoot {
        &self.root
    }

    pub fn as_path(&self) -> &Path {
        self.root.as_path()
    }
}

impl AsRef<Path> for LockedClone {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl super::GitManager {
    /// The clone, already locked. Prefer this to `ensure_repo_cloned` plus
    /// [`Self::lock_clone`] at any site that mutates the working tree.
    pub async fn locked_clone(&self, repo: &str) -> Result<LockedClone> {
        let root = self.ensure_repo_cloned(repo).await?;
        let guard = self.lock_clone(repo).await.lock_owned().await;
        Ok(LockedClone {
            root,
            _guard: guard,
        })
    }
}
