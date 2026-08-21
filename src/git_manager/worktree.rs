use anyhow::Result;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tracing::{info, warn};

/// RAII Guard for an Ephemeral Git Worktree.
/// Guarantees that the worktree is cleanly pruned and removed when dropped.
pub struct EphemeralWorktree {
    pub repo: String,
    pub pr_number: u64,
    pub worktree_path: PathBuf,
    pub repo_dir: PathBuf,
}

/// Synchronous bound for the `Drop` path.
///
/// `crate::exec::run_bounded` is `async` and cannot be awaited from `Drop`, but
/// an unbounded `std::process::Command::output()` in a destructor blocks the
/// dropping thread for as long as git hangs, which is exactly the failure mode
/// invariant I5 exists to prevent. Spawn, poll, and kill at the deadline so the
/// destructor still gets a timeout and reaps its child.
///
/// The caller previously discarded the `Output`, so stdout/stderr are sent to
/// `/dev/null` rather than captured; nothing observable changes.
fn run_sync_bounded(cmd: &mut std::process::Command, limit: Duration, what: &str) {
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            warn!("{} failed to run during drop: {}", what, e);
            return;
        }
    };

    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {}
            Err(e) => {
                warn!("{} could not be waited on during drop: {}", what, e);
                return;
            }
        }
        if Instant::now() >= deadline {
            warn!(
                "{} exceeded its {}s drop-path timeout and was killed",
                what,
                limit.as_secs()
            );
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

impl EphemeralWorktree {
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
            run_sync_bounded(
                &mut remove_cmd,
                crate::exec::ExecClass::Vcs.timeout(),
                "git worktree remove (drop)",
            );

            let _ = std::fs::remove_dir_all(&self.worktree_path);

            let mut prune_cmd = std::process::Command::new("git");
            prune_cmd
                .current_dir(&self.repo_dir)
                .args(["worktree", "prune"]);
            run_sync_bounded(
                &mut prune_cmd,
                crate::exec::ExecClass::Quick.timeout(),
                "git worktree prune (drop)",
            );
        }
    }
}
