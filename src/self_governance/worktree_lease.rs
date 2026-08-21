//! Leases over git worktrees.
//!
//! Anvil's reaper previously had exactly one worktree mechanism,
//! `git worktree prune`, which removes only administrative entries whose
//! working directory is *already gone*. A live audit found all 247 worktrees on
//! `oyatie` had their directories present on disk, so prune reclaimed
//! approximately zero of them while the daemon reported success ~8,640 times a
//! day.
//!
//! Reclaiming a worktree whose directory still exists needs two things prune
//! cannot supply:
//!
//!   1. **Ownership.** Wall-clock age is not a predicate. Under load an agent
//!      legitimately holds a worktree longer than any threshold you pick, and
//!      deleting it out from under the agent looks like a flaky build rather
//!      than a reaper bug. A lease records the PID that took it, so liveness
//!      can be *asserted* instead of guessed.
//!   2. **A bounded set.** The reaper must never go hunting the filesystem for
//!      things that look reclaimable. It reclaims what it leased, and nothing
//!      else. A worktree with no lease in this store is, by construction, out
//!      of scope.
//!
//! A lease is therefore the unit of authority: no lease, no reclaim.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A claim on one git worktree, held by one process, for a bounded time.
///
/// `created_at` and `ttl` are public so a caller (and a test) can construct a
/// backdated lease without sleeping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeLease {
    /// The repository the worktree belongs to. Reclaim is issued from here,
    /// because `git worktree remove` must run against the owning repo.
    pub repo_dir: PathBuf,
    /// The worktree's own directory.
    pub worktree_path: PathBuf,
    /// The process that took the lease. While it is alive the worktree is
    /// in use, no matter how old the lease is.
    pub owner_pid: u32,
    pub created_at: SystemTime,
    pub ttl: Duration,
}

impl WorktreeLease {
    /// Takes a lease on behalf of the current process.
    pub fn take(
        repo_dir: impl Into<PathBuf>,
        worktree_path: impl Into<PathBuf>,
        ttl: Duration,
    ) -> Self {
        Self {
            repo_dir: repo_dir.into(),
            worktree_path: worktree_path.into(),
            owner_pid: std::process::id(),
            created_at: SystemTime::now(),
            ttl,
        }
    }

    /// True once `created_at + ttl` is in the past.
    ///
    /// A clock that has moved backwards yields `false` (not expired), because
    /// the safe answer to "I cannot tell how old this is" is to keep it.
    pub fn is_expired(&self, now: SystemTime) -> bool {
        match now.duration_since(self.created_at) {
            Ok(age) => age > self.ttl,
            Err(_) => false,
        }
    }

    /// Stable identity of the lease: one lease per worktree path.
    ///
    /// Re-recording a lease for the same worktree replaces the previous one
    /// rather than accumulating duplicates.
    pub fn lease_id(&self) -> String {
        format!(
            "{:016x}",
            fnv1a64(self.worktree_path.to_string_lossy().as_bytes())
        )
    }
}

/// FNV-1a. Used only to derive a stable filename from a path; not a security
/// primitive, and never used to decide whether something may be deleted.
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// The on-disk shape. Deliberately explicit integers rather than a derived
/// `SystemTime` encoding, so the format is readable by an operator and stable
/// across serde versions.
#[derive(Debug, Serialize, Deserialize)]
struct LeaseRecord {
    repo_dir: PathBuf,
    worktree_path: PathBuf,
    owner_pid: u32,
    created_at_epoch_secs: u64,
    created_at_epoch_nanos: u32,
    ttl_secs: u64,
    ttl_nanos: u32,
}

impl From<&WorktreeLease> for LeaseRecord {
    fn from(lease: &WorktreeLease) -> Self {
        let since_epoch = lease
            .created_at
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        Self {
            repo_dir: lease.repo_dir.clone(),
            worktree_path: lease.worktree_path.clone(),
            owner_pid: lease.owner_pid,
            created_at_epoch_secs: since_epoch.as_secs(),
            created_at_epoch_nanos: since_epoch.subsec_nanos(),
            ttl_secs: lease.ttl.as_secs(),
            ttl_nanos: lease.ttl.subsec_nanos(),
        }
    }
}

impl From<LeaseRecord> for WorktreeLease {
    fn from(record: LeaseRecord) -> Self {
        Self {
            repo_dir: record.repo_dir,
            worktree_path: record.worktree_path,
            owner_pid: record.owner_pid,
            created_at: UNIX_EPOCH
                + Duration::new(record.created_at_epoch_secs, record.created_at_epoch_nanos),
            ttl: Duration::new(record.ttl_secs, record.ttl_nanos),
        }
    }
}

/// Durable set of outstanding leases, one JSON file per worktree.
///
/// The store is the reaper's entire universe of reclaimable worktrees. Its root
/// is supplied by the caller so a test can point it at a throwaway directory
/// and a sweep can never reach a developer's real scratch state.
#[derive(Debug, Clone)]
pub struct LeaseStore {
    root: PathBuf,
}

impl LeaseStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_path_buf(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, lease: &WorktreeLease) -> PathBuf {
        self.root.join(format!("{}.json", lease.lease_id()))
    }

    /// Persists a lease, replacing any previous lease on the same worktree.
    pub async fn record(&self, lease: &WorktreeLease) -> Result<()> {
        tokio::fs::create_dir_all(&self.root)
            .await
            .with_context(|| format!("creating lease store at {}", self.root.display()))?;
        let encoded = serde_json::to_vec_pretty(&LeaseRecord::from(lease))
            .context("encoding worktree lease")?;
        let path = self.path_for(lease);
        tokio::fs::write(&path, encoded)
            .await
            .with_context(|| format!("writing lease {}", path.display()))?;
        Ok(())
    }

    /// Drops a lease. Called after a successful reclaim, or when the worktree
    /// it describes has already vanished.
    ///
    /// A refused (dirty) worktree deliberately keeps its lease: the operator
    /// still needs to see it.
    pub async fn release(&self, lease: &WorktreeLease) -> Result<()> {
        let path = self.path_for(lease);
        match tokio::fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).with_context(|| format!("releasing lease {}", path.display())),
        }
    }

    /// Every outstanding lease. A missing store is an empty store, not an
    /// error: a fresh install has leased nothing.
    ///
    /// An individual unreadable or malformed file is skipped rather than
    /// failing the whole sweep -- one corrupt record must not stop the reaper
    /// from inspecting the rest.
    pub async fn load_all(&self) -> Result<Vec<WorktreeLease>> {
        let mut leases = Vec::new();
        let mut entries = match tokio::fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(leases),
            Err(e) => {
                return Err(e)
                    .with_context(|| format!("reading lease store {}", self.root.display()));
            }
        };
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(bytes) = tokio::fs::read(&path).await else {
                tracing::warn!("lease file {} could not be read; skipping", path.display());
                continue;
            };
            match serde_json::from_slice::<LeaseRecord>(&bytes) {
                Ok(record) => leases.push(WorktreeLease::from(record)),
                Err(e) => {
                    tracing::warn!("lease file {} is malformed ({e}); skipping", path.display())
                }
            }
        }
        leases.sort_by(|a, b| a.worktree_path.cmp(&b.worktree_path));
        Ok(leases)
    }
}

/// Why the reaper decided as it did. Carried into logs so a sweep that
/// reclaims nothing still explains itself, instead of being silent -- silence
/// was indistinguishable from health in the shipped reaper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseVerdict {
    /// The owner process is still running. Never reclaim, at any age.
    OwnerAlive,
    /// The owner is gone and the lease has outlived its TTL.
    ExpiredAndOwnerDead,
    /// The owner is gone before the TTL elapsed; the lease is orphaned.
    OwnerDead,
    /// The directory is already missing: nothing to remove, only git's
    /// administrative entry to prune.
    DirectoryMissing,
}

impl LeaseVerdict {
    /// Whether this verdict permits reclaim. Note this is a *permission*, not a
    /// decision: the dirty-tree check still runs afterwards and can veto.
    pub fn is_reclaimable(self) -> bool {
        matches!(self, Self::ExpiredAndOwnerDead | Self::OwnerDead)
    }
}

/// Decides a lease's fate.
///
/// The rule is "reclaimable when the owner is dead OR the lease has expired",
/// with one hard veto in front of it: **a live owner is never reclaimed**. An
/// expired lease held by a running process means the work is taking longer than
/// budgeted, not that the work is abandoned.
pub fn judge(lease: &WorktreeLease, now: SystemTime, owner_alive: bool) -> LeaseVerdict {
    if !lease.worktree_path.exists() {
        return LeaseVerdict::DirectoryMissing;
    }
    if owner_alive {
        return LeaseVerdict::OwnerAlive;
    }
    if lease.is_expired(now) {
        LeaseVerdict::ExpiredAndOwnerDead
    } else {
        LeaseVerdict::OwnerDead
    }
}

/// Whether `pid` names a running process.
///
/// On failure this answers **true**. The consequence of a false "alive" is a
/// worktree kept one sweep longer; the consequence of a false "dead" is
/// deleting a working tree from under a running agent. The asymmetry decides
/// the default.
pub async fn process_is_alive(pid: u32) -> bool {
    if pid == 0 {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        if Path::new("/proc").is_dir() {
            return Path::new(&format!("/proc/{pid}")).exists();
        }
    }
    let mut cmd = tokio::process::Command::new("ps");
    cmd.args(["-p", &pid.to_string(), "-o", "pid="]);
    cmd.stdin(std::process::Stdio::null());
    match crate::exec::run_bounded(cmd, crate::exec::ExecClass::Quick, "ps -p").await {
        Ok(out) => out.status.success(),
        Err(e) => {
            tracing::warn!("liveness probe for pid {pid} failed ({e}); assuming alive");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_backdated_lease_is_expired() {
        let lease = WorktreeLease {
            repo_dir: PathBuf::from("/nonexistent/repo"),
            worktree_path: PathBuf::from("/nonexistent/wt"),
            owner_pid: 1,
            created_at: SystemTime::now() - Duration::from_secs(7200),
            ttl: Duration::from_secs(3600),
        };
        assert!(lease.is_expired(SystemTime::now()));
    }

    #[test]
    fn a_fresh_lease_is_not_expired() {
        let lease = WorktreeLease::take(
            "/nonexistent/repo",
            "/nonexistent/wt",
            Duration::from_secs(3600),
        );
        assert!(!lease.is_expired(SystemTime::now()));
    }

    #[test]
    fn a_live_owner_vetoes_reclaim_even_when_expired() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lease = WorktreeLease {
            repo_dir: tmp.path().to_path_buf(),
            worktree_path: tmp.path().to_path_buf(),
            owner_pid: std::process::id(),
            created_at: SystemTime::now() - Duration::from_secs(7200),
            ttl: Duration::from_secs(3600),
        };
        let verdict = judge(&lease, SystemTime::now(), true);
        assert_eq!(verdict, LeaseVerdict::OwnerAlive);
        assert!(!verdict.is_reclaimable());
    }

    #[test]
    fn a_dead_owner_over_an_expired_lease_is_reclaimable() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let lease = WorktreeLease {
            repo_dir: tmp.path().to_path_buf(),
            worktree_path: tmp.path().to_path_buf(),
            owner_pid: 424242,
            created_at: SystemTime::now() - Duration::from_secs(7200),
            ttl: Duration::from_secs(3600),
        };
        let verdict = judge(&lease, SystemTime::now(), false);
        assert_eq!(verdict, LeaseVerdict::ExpiredAndOwnerDead);
        assert!(verdict.is_reclaimable());
    }

    #[tokio::test]
    async fn this_process_reads_as_alive() {
        assert!(process_is_alive(std::process::id()).await);
    }

    #[tokio::test]
    async fn a_store_round_trips_a_lease() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = LeaseStore::new(tmp.path().join("leases"));
        assert!(store.load_all().await.expect("empty store").is_empty());

        let lease = WorktreeLease {
            repo_dir: PathBuf::from("/repo"),
            worktree_path: PathBuf::from("/repo/wt/a"),
            owner_pid: 99,
            created_at: SystemTime::now() - Duration::from_secs(10),
            ttl: Duration::from_secs(60),
        };
        store.record(&lease).await.expect("record");
        let loaded = store.load_all().await.expect("load");
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].worktree_path, lease.worktree_path);
        assert_eq!(loaded[0].owner_pid, 99);
        assert_eq!(loaded[0].ttl, Duration::from_secs(60));

        // Re-recording the same worktree replaces rather than duplicates.
        store.record(&lease).await.expect("re-record");
        assert_eq!(store.load_all().await.expect("load").len(), 1);

        store.release(&lease).await.expect("release");
        assert!(store.load_all().await.expect("load").is_empty());
    }
}
