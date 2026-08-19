use anyhow::Result;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::info;

/// RAII Guard for an Ephemeral Git Worktree.
/// Guarantees that the worktree is cleanly pruned and removed when dropped.
pub struct EphemeralWorktree {
    pub repo: String,
    pub pr_number: u64,
    pub worktree_path: PathBuf,
    pub repo_dir: PathBuf,
}

impl EphemeralWorktree {
    /// Explicit asynchronous cleanup of the ephemeral worktree
    pub async fn cleanup(&self) -> Result<()> {
        info!(
            "EphemeralWorktree: Cleaning up worktree at {:?} for {}#{}",
            self.worktree_path, self.repo, self.pr_number
        );

        let _ = Command::new("git")
            .current_dir(&self.repo_dir)
            .args([
                "worktree",
                "remove",
                "--force",
                self.worktree_path.to_str().unwrap(),
            ])
            .output()
            .await;

        if self.worktree_path.exists() {
            let _ = tokio::fs::remove_dir_all(&self.worktree_path).await;
        }

        let _ = Command::new("git")
            .current_dir(&self.repo_dir)
            .args(["worktree", "prune"])
            .output()
            .await;

        Ok(())
    }
}

impl Drop for EphemeralWorktree {
    fn drop(&mut self) {
        if self.worktree_path.exists() {
            // Synchronous fallback cleanup in case async cleanup was not called
            let _ = std::process::Command::new("git")
                .current_dir(&self.repo_dir)
                .args([
                    "worktree",
                    "remove",
                    "--force",
                    self.worktree_path.to_str().unwrap(),
                ])
                .output();

            let _ = std::fs::remove_dir_all(&self.worktree_path);

            let _ = std::process::Command::new("git")
                .current_dir(&self.repo_dir)
                .args(["worktree", "prune"])
                .output();
        }
    }
}
