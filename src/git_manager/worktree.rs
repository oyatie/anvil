use anyhow::Result;
use std::path::PathBuf;

use super::subject::SubjectRoot;
use tokio::process::Command;
use tracing::info;

/// RAII Guard for an Ephemeral Git Worktree.
/// Guarantees that the worktree is cleanly pruned and removed when dropped.
pub struct EphemeralWorktree {
    pub repo: String,
    pub pr_number: u64,
    pub worktree_path: PathBuf,
    pub repo_dir: SubjectRoot,
}

impl EphemeralWorktree {
    /// Whether this tree really is the commit a caller is about to describe it
    /// as.
    ///
    /// `GitManager::create_ephemeral_worktree` asks git for `head_sha` and
    /// falls back to `FETCH_HEAD` when the object is not local yet, and
    /// `FETCH_HEAD` in the shared clone is whatever ref was fetched last -- the
    /// base branch the queue healer just fetched, or another pull request's
    /// head fetched concurrently by `prepare_pr_diff` for a different pull
    /// request of the same repository. Nothing about the worktree records which
    /// of the two happened, so a gate run in it and published as a measurement
    /// of `head_sha` may be a measurement of a different commit.
    ///
    /// One `rev-parse` closes that. `Err` carries what the tree is actually at,
    /// so the caller can withhold and name the reason rather than publish a
    /// measurement of the wrong commit.
    pub async fn verify_at(&self, head_sha: &str) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.worktree_path)
            .args(["rev-parse", "HEAD"]);
        let out = crate::exec::run_bounded(
            cmd,
            crate::exec::ExecClass::Quick,
            "git rev-parse HEAD (ephemeral worktree)",
        )
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "the worktree for {}#{} could not be asked which commit it is at: {:#}",
                self.repo,
                self.pr_number,
                e
            )
        })?;
        if !out.status.success() {
            anyhow::bail!(
                "the worktree for {}#{} could not be asked which commit it is at: {}",
                self.repo,
                self.pr_number,
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        let at = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if at != head_sha {
            anyhow::bail!(
                "the worktree for {}#{} was asked for {} and is at {}: git fell back to \
                 FETCH_HEAD in the shared clone, which is whatever was fetched last. A tree \
                 Anvil cannot prove is the certified commit is not a measurement of it.",
                self.repo,
                self.pr_number,
                head_sha,
                at
            );
        }
        Ok(())
    }

    /// The tree, proven to be at `head_sha`, as a type a gate can be handed.
    ///
    /// [`Self::verify_at`] answers the question; this carries the answer. A
    /// gate taking [`CertifiedTree`] cannot be given the shared clone, which
    /// is never checked out at the head under review -- so a filesystem read
    /// inside it is a read of this pull request rather than of whichever one
    /// the fixer last touched.
    pub async fn verified_at(&self, head_sha: &str) -> Result<crate::git_manager::CertifiedTree> {
        self.verify_at(head_sha).await?;
        Ok(crate::git_manager::CertifiedTree::proven(
            self.repo_dir.clone(),
            head_sha.to_string(),
        ))
    }

    /// Explicit asynchronous cleanup of the ephemeral worktree
    pub async fn cleanup(&self) -> Result<()> {
        info!(
            "EphemeralWorktree: Cleaning up worktree at {:?} for {}#{}",
            self.worktree_path, self.repo, self.pr_number
        );

        let mut remove_cmd = Command::new("git");
        remove_cmd.current_dir(&self.repo_dir).args([
            "worktree",
            "remove",
            "--force",
            self.worktree_path.to_str().unwrap(),
        ]);
        let _ = crate::exec::run_bounded(
            remove_cmd,
            crate::exec::ExecClass::Vcs,
            "git worktree remove",
        )
        .await;

        if self.worktree_path.exists() {
            let _ = tokio::fs::remove_dir_all(&self.worktree_path).await;
        }

        let mut prune_cmd = Command::new("git");
        prune_cmd
            .current_dir(&self.repo_dir)
            .args(["worktree", "prune"]);
        let _ = crate::exec::run_bounded(
            prune_cmd,
            crate::exec::ExecClass::Quick,
            "git worktree prune",
        )
        .await;

        Ok(())
    }
}

impl Drop for EphemeralWorktree {
    fn drop(&mut self) {
        if self.worktree_path.exists() {
            // Synchronous fallback cleanup in case async cleanup was not called
            let mut remove_cmd = std::process::Command::new("git");
            remove_cmd.current_dir(&self.repo_dir).args([
                "worktree",
                "remove",
                "--force",
                self.worktree_path.to_str().unwrap(),
            ]);
            if let Err(error) = crate::exec::run_sync_bounded(
                remove_cmd,
                crate::exec::ExecClass::Vcs.timeout(),
                "git worktree remove (drop)",
            ) {
                tracing::warn!("git worktree removal during drop failed: {error:#}");
            }

            let _ = std::fs::remove_dir_all(&self.worktree_path);

            let mut prune_cmd = std::process::Command::new("git");
            prune_cmd
                .current_dir(&self.repo_dir)
                .args(["worktree", "prune"]);
            if let Err(error) = crate::exec::run_sync_bounded(
                prune_cmd,
                crate::exec::ExecClass::Quick.timeout(),
                "git worktree prune (drop)",
            ) {
                tracing::warn!("git worktree prune during drop failed: {error:#}");
            }
        }
    }
}

impl super::GitManager {
    /// A worktree at `head_sha`, proven to be there.
    ///
    /// Both certification paths need it and neither may fall back to the shared
    /// clone: that clone is never checked out at the head under review, so a
    /// filesystem-reading gate in it measures the base branch or whichever pull
    /// request the fixer last touched. The report would still carry a genuine
    /// provenance mark and a subject naming this head, so `subject_refusal`
    /// admits it and Anvil signs an approval over a tree it never read.
    ///
    /// `Err` withholds the whole certification. A withheld certification is
    /// retried; a certification of the wrong tree is signed.
    pub async fn certified_tree_at(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
    ) -> Result<super::CertifiedTree> {
        let worktree = self
            .create_ephemeral_worktree(repo, pr_number, head_sha)
            .await
            .map_err(|e| {
                anyhow::anyhow!(
                    "no tree at {head_sha} for {repo}#{pr_number}, so nothing was certified: {e:#}"
                )
            })?;
        match worktree.verified_at(head_sha).await {
            Ok(tree) => Ok(tree),
            Err(e) => {
                let _ = worktree.cleanup().await;
                Err(e)
            }
        }
    }
}
