//! Lane worktrees over git. Every subprocess is bounded (I5); there is no
//! code path that pushes, forces, or skips hooks — pushing belongs to the
//! landing step and is not implemented here.

use crate::change_delivery::ports::{LaneError, LaneWorktree, NameStatus, VcsPort};
use crate::exec::{ExecClass, run_bounded};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;

pub const LANE_LEASE_FILE: &str = ".anvil-lane-lease";

/// Artefact directory kept out of every lane commit.
///
/// A lane commit carries what the change produced, never the harness's own
/// provenance output. Declared here because it is a property of the lane
/// protocol; the subsystem that writes into it is free to change or disappear.
const LANE_EXCLUDED_RECEIPTS_DIR: &str = ".anvil/receipts";
const LANE_LEASE: Duration = Duration::from_secs(3600);

pub struct GitLaneVcs {
    worktrees_base: PathBuf,
}

impl GitLaneVcs {
    pub fn new(worktrees_base: PathBuf) -> Self {
        GitLaneVcs { worktrees_base }
    }

    async fn git(dir: &Path, args: &[&str], what: &str) -> Result<std::process::Output, LaneError> {
        let mut cmd = Command::new("git");
        cmd.current_dir(dir).args(args);
        run_bounded(cmd, ExecClass::Vcs, what)
            .await
            .map_err(|e| LaneError::Failed(e.to_string()))
    }

    async fn git_ok(
        dir: &Path,
        args: &[&str],
        what: &str,
    ) -> Result<std::process::Output, LaneError> {
        let out = Self::git(dir, args, what).await?;
        if !out.status.success() {
            return Err(LaneError::Failed(format!(
                "{what}: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(out)
    }
}

#[async_trait]
impl VcsPort for GitLaneVcs {
    async fn create_lane(
        &self,
        repo_dir: &Path,
        lane_id: &str,
        base_rev: &str,
        allow_same_repo: bool,
    ) -> Result<LaneWorktree, LaneError> {
        if !allow_same_repo {
            super::self_source_guard::assert_not_daemon_tree(repo_dir)
                .await
                .map_err(LaneError::Refused)?;
        }
        if base_rev.len() != 40 || !base_rev.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(LaneError::Refused(format!(
                "lane base must be a full sha, got {base_rev:?}"
            )));
        }
        let safe: String = lane_id
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if safe.is_empty() {
            return Err(LaneError::Refused("empty lane id".into()));
        }
        let path = self.worktrees_base.join(format!("lane-{safe}"));
        if path.exists() {
            return Err(LaneError::Refused(format!(
                "lane worktree {} already exists (one lane, one worktree)",
                path.display()
            )));
        }
        tokio::fs::create_dir_all(&self.worktrees_base)
            .await
            .map_err(|e| LaneError::Failed(e.to_string()))?;
        Self::git_ok(
            repo_dir,
            &[
                "worktree",
                "add",
                "--detach",
                path.to_str().unwrap_or_default(),
                base_rev,
            ],
            "git worktree add (lane)",
        )
        .await?;
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + LANE_LEASE.as_secs();
        let _ = tokio::fs::write(path.join(LANE_LEASE_FILE), format!("{expiry}\n")).await;
        Ok(LaneWorktree {
            lane_id: safe,
            path,
            base_rev: base_rev.to_string(),
        })
    }

    async fn apply_move(&self, lane: &LaneWorktree, from: &str, to: &str) -> Result<(), LaneError> {
        if from.is_empty() || to.is_empty() {
            return Err(LaneError::Refused("move with an empty endpoint".into()));
        }
        if let Some(parent) = Path::new(to).parent() {
            let _ = tokio::fs::create_dir_all(lane.path.join(parent)).await;
        }
        Self::git_ok(
            &lane.path,
            &["mv", from.trim_end_matches('/'), to.trim_end_matches('/')],
            "git mv (lane)",
        )
        .await
        .map(|_| ())
    }

    async fn stage(&self, lane: &LaneWorktree) -> Result<(), LaneError> {
        // Excluded as a repo-layout fact, not by importing the writer's constant.
        //
        // This read `crate::attestation_guard::ANVIL_RECEIPTS_DIR`, which made a
        // Migrating component depend on a Superseded one -- the migration
        // boundary gate caught it. The deeper problem is that a VCS adapter
        // should not know what Anvil writes to disk: what belongs out of a lane
        // commit is a property of the lane, not of whichever subsystem happens
        // to produce the artefacts.
        let receipts = format!(":(exclude){LANE_EXCLUDED_RECEIPTS_DIR}");
        let lease = format!(":(exclude){LANE_LEASE_FILE}");
        Self::git_ok(
            &lane.path,
            &["add", "-A", "--", ".", receipts.as_str(), lease.as_str()],
            "git add (lane)",
        )
        .await
        .map(|_| ())
    }

    async fn name_status(&self, lane: &LaneWorktree) -> Result<Vec<NameStatus>, LaneError> {
        let out = Self::git_ok(
            &lane.path,
            &["diff", "--cached", "-M50%", "--name-status"],
            "git diff --name-status (lane)",
        )
        .await?;
        Ok(NameStatus::parse(&String::from_utf8_lossy(&out.stdout)))
    }

    async fn cached_diff(&self, lane: &LaneWorktree) -> Result<String, LaneError> {
        let out = Self::git_ok(
            &lane.path,
            &["diff", "--cached", "-M50%"],
            "git diff --cached (lane)",
        )
        .await?;
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    async fn diffstat(&self, lane: &LaneWorktree) -> Result<String, LaneError> {
        let out = Self::git_ok(
            &lane.path,
            &["diff", "--cached", "-M50%", "--stat"],
            "git diff --stat (lane)",
        )
        .await?;
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    }

    async fn cleanup(&self, lane: LaneWorktree) -> Result<(), LaneError> {
        // No `worktree remove --force`: resolve the owning repository first,
        // delete the lane directory, and prune the stale administrative
        // entry — the same mechanism the GC uses, with no force verb at all.
        let owner = Self::git(
            &lane.path,
            &["rev-parse", "--git-common-dir"],
            "git rev-parse (lane cleanup)",
        )
        .await
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        let _ = tokio::fs::remove_dir_all(&lane.path).await;
        if let Some(common) = owner
            && let Some(repo) = std::path::Path::new(&common).parent()
        {
            let _ = Self::git(
                repo,
                &["worktree", "prune"],
                "git worktree prune (lane cleanup)",
            )
            .await;
        }
        Ok(())
    }
}
