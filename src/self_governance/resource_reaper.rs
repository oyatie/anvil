//! Autonomous resource reaper.
//!
//! # What was wrong
//!
//! The previous `run_sweep` took `repo_dir: Option<&Path>` and put its entire
//! worktree path behind `if let Some(dir) = repo_dir`. The only caller,
//! `SelfGovernor::spawn_monitoring_daemon`, passed `None` on a 10s interval, so
//! roughly 8,640 times a day the sweep took the dead branch and inspected no
//! worktree at all. On the live branch the sole mechanism was
//! `git worktree prune`, which by definition drops only administrative entries
//! whose working directory is *already missing*; a live audit found all 247
//! worktrees on `oyatie` still had their directories, so prune was a no-op
//! against every one of them -- while the code checked prune's exit status and
//! counted a success, reporting work for zero work.
//!
//! # What replaces it
//!
//! `run_sweep` now takes no repo argument at all, so the `None` branch is
//! structurally impossible to reintroduce. Worktrees are reclaimed through
//! *leases* (see [`crate::self_governance::worktree_lease`]): the reaper
//! inspects exactly the worktrees it leased, and reclaim goes through
//! `git worktree remove` rather than prune.
//!
//! # Safety
//!
//! `--force` is never passed. A live audit found 80 of 159 stale worktrees
//! holding 4,332 tracked-file modifications that existed in no commit, no stash
//! and no remote; `git worktree remove --force` would have destroyed all of it
//! irrecoverably. A dirty worktree is refused and reported in
//! [`GarbageCollectionReport::worktrees_refused_dirty`] so an operator can
//! rescue the work, and its lease is deliberately kept so it keeps being
//! reported.
//!
//! # Default posture
//!
//! Worktree reclaim is **off unless a lease store is supplied**. The daemon
//! constructs the reaper through [`AutonomousResourceReaper::default`], which
//! has no lease store, so the scheduled sweep still only touches Anvil's own
//! scratch directories. Reclaim is armed explicitly, per call site, with
//! [`AutonomousResourceReaper::with_lease_store`].

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{info, warn};

use super::worktree_lease::{judge, process_is_alive, LeaseStore, LeaseVerdict, WorktreeLease};

#[derive(Debug, Clone, Default)]
pub struct GarbageCollectionReport {
    /// Leases whose worktree directory was already gone, so only git's
    /// administrative entry remained to prune.
    pub orphaned_worktrees_pruned: usize,
    /// Every lease the sweep looked at. A sweep that inspects zero worktrees
    /// is the shipped defect; this is the number that made it visible.
    pub worktrees_inspected: usize,
    /// Worktrees actually removed from disk *and* from `git worktree list`.
    pub worktrees_reclaimed: Vec<PathBuf>,
    /// Reclaimable by lease, but holding uncommitted work. Never removed.
    pub worktrees_refused_dirty: Vec<PathBuf>,
    pub temporary_directories_reaped: usize,
    pub freed_bytes_estimate: u64,
}

pub struct AutonomousResourceReaper {
    staging_dirs: Vec<PathBuf>,
    /// `None` means worktree reclaim is disabled for this reaper.
    lease_store: Option<LeaseStore>,
}

impl Default for AutonomousResourceReaper {
    /// Anvil's own scratch directories, and **no** worktree reclaim.
    ///
    /// These are Anvil's staging areas, not managed repositories: a managed
    /// repo's worktrees live elsewhere and are out of scope for the scheduled
    /// sweep.
    fn default() -> Self {
        Self::new(vec![
            std::env::temp_dir().join("anvil"),
            std::env::temp_dir().join("anvil_worktrees"),
            std::env::temp_dir().join("anvil_staging"),
        ])
    }
}

impl AutonomousResourceReaper {
    /// A reaper that sweeps scratch directories only. Worktree reclaim is off.
    pub fn new(staging_dirs: Vec<PathBuf>) -> Self {
        Self {
            staging_dirs,
            lease_store: None,
        }
    }

    /// Arms worktree reclaim against the worktrees recorded in `store`, and
    /// only those. A worktree with no lease in this store is out of scope no
    /// matter how stale it looks.
    pub fn with_lease_store(staging_dirs: Vec<PathBuf>, store: LeaseStore) -> Self {
        Self {
            staging_dirs,
            lease_store: Some(store),
        }
    }

    /// Autonomous background garbage collection.
    ///
    /// Takes no repository argument: the set of worktrees under consideration
    /// comes from the lease store, so there is no `None` case to fall through.
    pub async fn run_sweep(&self) -> Result<GarbageCollectionReport> {
        let mut report = GarbageCollectionReport::default();

        // 1. Reclaim leased worktrees whose holder is gone.
        if let Some(store) = &self.lease_store {
            self.sweep_leases(store, &mut report).await;
        }

        // 2. Clean temporary scratch directories older than 1 hour.
        let now = SystemTime::now();
        for dir in &self.staging_dirs {
            if dir.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let path = entry.path();
                        if let Ok(meta) = entry.metadata().await {
                            if let Ok(modified) = meta.modified() {
                                if let Ok(age) = now.duration_since(modified) {
                                    if age > std::time::Duration::from_secs(3600) {
                                        let _ = tokio::fs::remove_dir_all(&path).await;
                                        report.temporary_directories_reaped += 1;
                                        report.freed_bytes_estimate += meta.len();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if report.temporary_directories_reaped > 0
            || report.orphaned_worktrees_pruned > 0
            || !report.worktrees_reclaimed.is_empty()
            || !report.worktrees_refused_dirty.is_empty()
        {
            info!(
                "🧹 [Autonomous Resource Reaper] inspected {} leased worktrees: {} reclaimed, {} refused (dirty), {} pruned (directory already gone); {} temp directories reaped (~{} bytes)",
                report.worktrees_inspected,
                report.worktrees_reclaimed.len(),
                report.worktrees_refused_dirty.len(),
                report.orphaned_worktrees_pruned,
                report.temporary_directories_reaped,
                report.freed_bytes_estimate
            );
        }

        Ok(report)
    }

    async fn sweep_leases(&self, store: &LeaseStore, report: &mut GarbageCollectionReport) {
        let leases = match store.load_all().await {
            Ok(leases) => leases,
            Err(e) => {
                warn!(
                    "lease store {} could not be read ({e}); no worktree was inspected this sweep",
                    store.root().display()
                );
                return;
            }
        };

        let now = SystemTime::now();
        for lease in leases {
            report.worktrees_inspected += 1;
            let alive = process_is_alive(lease.owner_pid).await;
            match judge(&lease, now, alive) {
                LeaseVerdict::OwnerAlive => {
                    // The lease may well be expired. A running holder still
                    // wins: over-running a budget is not abandonment.
                }
                LeaseVerdict::DirectoryMissing => {
                    // The one case `git worktree prune` was ever right about.
                    if self.prune_repo(&lease.repo_dir).await {
                        report.orphaned_worktrees_pruned += 1;
                    }
                    if let Err(e) = store.release(&lease).await {
                        warn!("could not release lease for missing worktree: {e}");
                    }
                }
                verdict @ (LeaseVerdict::ExpiredAndOwnerDead | LeaseVerdict::OwnerDead) => {
                    debug_assert!(verdict.is_reclaimable());
                    self.reclaim(store, &lease, report).await;
                }
            }
        }
    }

    /// Removes one leased worktree, refusing outright if it holds work.
    async fn reclaim(
        &self,
        store: &LeaseStore,
        lease: &WorktreeLease,
        report: &mut GarbageCollectionReport,
    ) {
        match self.worktree_is_dirty(&lease.worktree_path).await {
            // Unknown state is treated as dirty. If the reaper cannot prove a
            // worktree is clean it must not delete it.
            None => {
                warn!(
                    "could not determine cleanliness of {}; refusing to reclaim",
                    lease.worktree_path.display()
                );
                report
                    .worktrees_refused_dirty
                    .push(lease.worktree_path.clone());
                return;
            }
            Some(true) => {
                warn!(
                    "⚠️  [Resource Reaper] {} holds uncommitted work and will NOT be removed; rescue it or commit it",
                    lease.worktree_path.display()
                );
                report
                    .worktrees_refused_dirty
                    .push(lease.worktree_path.clone());
                // Lease intentionally retained: a refused worktree must keep
                // showing up in reports until a human deals with it.
                return;
            }
            Some(false) => {}
        }

        // No `--force`, ever. Plain `remove` keeps git's own dirty-tree refusal
        // in the loop as a second line of defence behind the status check.
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("-C")
            .arg(&lease.repo_dir)
            .args(["worktree", "remove"])
            .arg(&lease.worktree_path)
            .stdin(std::process::Stdio::null());

        match crate::exec::run_bounded(cmd, crate::exec::ExecClass::Vcs, "git worktree remove")
            .await
        {
            Ok(out) if out.status.success() => {
                report.worktrees_reclaimed.push(lease.worktree_path.clone());
                if let Err(e) = store.release(lease).await {
                    warn!("worktree reclaimed but lease could not be released: {e}");
                }
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stderr.contains("modified or untracked") {
                    // git caught what the status probe did not. Report it as
                    // the safety refusal it is rather than a generic failure.
                    report
                        .worktrees_refused_dirty
                        .push(lease.worktree_path.clone());
                }
                warn!(
                    "git worktree remove refused {}: {}",
                    lease.worktree_path.display(),
                    stderr.trim()
                );
            }
            Err(e) => warn!(
                "git worktree remove failed for {}: {e}",
                lease.worktree_path.display()
            ),
        }
    }

    /// `Some(true)` dirty, `Some(false)` clean, `None` undeterminable.
    ///
    /// `--porcelain` reports both modified tracked files and untracked files,
    /// which is exactly the set `git worktree remove` refuses on.
    async fn worktree_is_dirty(&self, worktree: &Path) -> Option<bool> {
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("-C")
            .arg(worktree)
            .args(["status", "--porcelain"])
            .stdin(std::process::Stdio::null());
        let out = crate::exec::run_bounded(cmd, crate::exec::ExecClass::Quick, "git status")
            .await
            .ok()?;
        if !out.status.success() {
            return None;
        }
        Some(!out.stdout.is_empty())
    }

    async fn prune_repo(&self, repo_dir: &Path) -> bool {
        if !repo_dir.join(".git").exists() {
            return false;
        }
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("-C")
            .arg(repo_dir)
            .args(["worktree", "prune"])
            .stdin(std::process::Stdio::null());
        matches!(
            crate::exec::run_bounded(cmd, crate::exec::ExecClass::Quick, "git worktree prune").await,
            Ok(out) if out.status.success()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resource_reaper_sweeps_temp() {
        let temp_base = std::env::temp_dir().join("anvil_test_reaper");
        let _ = tokio::fs::create_dir_all(&temp_base).await;

        let reaper = AutonomousResourceReaper::new(vec![temp_base.clone()]);
        let rep = reaper.run_sweep().await;
        assert!(rep.is_ok());

        let _ = tokio::fs::remove_dir_all(&temp_base).await;
    }

    /// Without a lease store the sweep must not touch a worktree at all --
    /// this is the "off by default" posture the daemon runs under.
    #[tokio::test]
    async fn a_reaper_without_a_lease_store_inspects_no_worktree() {
        let reaper = AutonomousResourceReaper::new(vec![]);
        let report = reaper.run_sweep().await.expect("sweep");
        assert_eq!(report.worktrees_inspected, 0);
        assert!(report.worktrees_reclaimed.is_empty());
        assert!(report.worktrees_refused_dirty.is_empty());
    }
}
