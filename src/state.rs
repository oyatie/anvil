use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::{Mutex, RwLock};
use tracing::info;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PrState {
    pub last_reviewed_head_sha: String,
    pub last_reviewed_at: String,
    pub review_count: u32,
    pub last_review_verdict: Option<String>,
    pub last_certified_head_sha: Option<String>,
    pub is_enlisted_in_merge_queue: bool,
    /// How many times anvil has rewritten this pull request in response to its
    /// own review. Bounded by `next_phase::MAX_AUTO_FIX_ATTEMPTS`.
    ///
    /// `serde(default)` is load-bearing, not decoration: `StateManager::load`
    /// refuses to start when `pr_states.json` does not parse, so a new field
    /// without a default would take the daemon down on a state file written by
    /// any earlier build.
    #[serde(default)]
    pub auto_fix_attempts: u32,
    /// The head the fixer last ran against.
    ///
    /// A fixer run that pushes nothing leaves the head where it was, so this is
    /// what stops a fixer that cannot satisfy the review from running forever.
    #[serde(default)]
    pub last_auto_fixed_head_sha: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WalEntry {
    pub timestamp: String,
    pub key: String,
    pub state: PrState,
}

#[derive(Debug)]
pub struct StateManager {
    file_path: PathBuf,
    wal_path: PathBuf,
    states: RwLock<HashMap<String, PrState>>,
    locks: RwLock<HashMap<String, Arc<Mutex<()>>>>,
}

impl StateManager {
    pub async fn load(data_dir: &Path) -> Result<Self> {
        tokio::fs::create_dir_all(data_dir)
            .await
            .context("Failed to create data directory")?;

        let file_path = data_dir.join("pr_states.json");
        let wal_path = data_dir.join("pr_states.wal");

        let mut states: HashMap<String, PrState> = if file_path.exists() {
            let content = tokio::fs::read_to_string(&file_path)
                .await
                .context("Failed to read pr_states.json")?;
            serde_json::from_str(&content)
                .context("pr_states.json did not parse; refusing to start with empty state")?
        } else {
            HashMap::new()
        };

        // Replay WAL journal if present (Crash Recovery)
        if wal_path.exists()
            && let Ok(wal_content) = tokio::fs::read_to_string(&wal_path).await
        {
            let mut replayed_count = 0;
            for line in wal_content.lines() {
                if let Ok(entry) = serde_json::from_str::<WalEntry>(line) {
                    states.insert(entry.key, entry.state);
                    replayed_count += 1;
                }
            }
            if replayed_count > 0 {
                info!(
                    "🔄 [State WAL Replay] Successfully recovered {} uncheckpointed state mutations from WAL.",
                    replayed_count
                );
            }
        }

        Ok(Self {
            file_path,
            wal_path,
            states: RwLock::new(states),
            locks: RwLock::new(HashMap::new()),
        })
    }

    /// Obtains an exclusive per-PR lock to prevent concurrent TOCTOU races
    pub async fn acquire_pr_lock(&self, repo: &str, pr_number: u64) -> Arc<Mutex<()>> {
        let key = Self::key(repo, pr_number);
        {
            let locks_read = self.locks.read().await;
            if let Some(lock) = locks_read.get(&key) {
                return lock.clone();
            }
        }
        let mut locks_write = self.locks.write().await;
        locks_write
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn key(repo: &str, pr_number: u64) -> String {
        format!("{}#{}", repo.to_lowercase(), pr_number)
    }

    pub async fn get_pr_state(&self, repo: &str, pr_number: u64) -> Option<PrState> {
        let key = Self::key(repo, pr_number);
        let states = self.states.read().await;
        states.get(&key).cloned()
    }

    pub async fn update_pr_state(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: String,
        verdict: Option<String>,
    ) -> Result<PrState> {
        let key = Self::key(repo, pr_number);
        let mut states = self.states.write().await;

        let entry = states.entry(key.clone()).or_default();
        entry.last_reviewed_head_sha = head_sha;
        entry.last_reviewed_at = chrono_iso_now();
        entry.review_count += 1;
        entry.last_review_verdict = verdict;

        let updated = entry.clone();

        // 1. Append-only WAL entry with immediate flush
        let wal_entry = WalEntry {
            timestamp: entry.last_reviewed_at.clone(),
            key: key.clone(),
            state: updated.clone(),
        };
        self.append_wal(&wal_entry).await?;

        // 2. Atomic checkpoint with temp file + sync_all + atomic rename(2)
        self.atomic_checkpoint(&states).await?;

        Ok(updated)
    }

    /// Clears the reviewed-SHA stamp so the pull request is re-reviewed.
    ///
    /// `update_pr_state` stamps `last_reviewed_head_sha` early in the pipeline,
    /// and the early-exit guard in the review pipeline skips any webhook whose
    /// head SHA matches it. A pipeline that aborts *after* stamping therefore
    /// strands the PR: it is never retried, because every later delivery for
    /// that SHA is treated as already reviewed.
    ///
    /// The merge decision stays safe -- no certification is posted, so branch
    /// protection still blocks -- but the PR silently stops progressing until a
    /// new commit lands. Abort paths call this so a transient failure costs a
    /// retry rather than the whole review.
    pub async fn clear_reviewed_sha(&self, repo: &str, pr_number: u64) {
        let key = Self::key(repo, pr_number);
        let states = {
            let mut states = self.states.write().await;
            match states.get_mut(&key) {
                Some(entry) => entry.last_reviewed_head_sha.clear(),
                None => return,
            }
            states.clone()
        };
        // Best-effort durability: if the checkpoint fails the in-memory clear
        // still allows a retry within this process lifetime.
        if let Err(e) = self.atomic_checkpoint(&states).await {
            tracing::warn!(
                "Could not persist reviewed-SHA rollback for {}#{}: {}",
                repo,
                pr_number,
                e
            );
        }
    }

    pub async fn record_certification(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
        enlisted: bool,
    ) -> Result<()> {
        let key = Self::key(repo, pr_number);
        let mut states = self.states.write().await;
        let entry = states.entry(key.clone()).or_default();
        entry.last_certified_head_sha = Some(head_sha.to_string());
        entry.is_enlisted_in_merge_queue = enlisted;

        let wal_entry = WalEntry {
            timestamp: chrono_iso_now(),
            key: key.clone(),
            state: entry.clone(),
        };
        self.append_wal(&wal_entry).await?;
        self.atomic_checkpoint(&states).await?;
        Ok(())
    }

    /// Appends transaction to WAL file with immediate fdatasync
    async fn append_wal(&self, entry: &WalEntry) -> Result<()> {
        if let Ok(serialized) = serde_json::to_string(entry) {
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.wal_path)
                .await?;
            file.write_all(serialized.as_bytes()).await?;
            file.write_all(b"\n").await?;
            file.sync_data().await?; // fdatasync
        }
        Ok(())
    }

    /// Performs atomic POSIX temporary-file rename checkpoint
    async fn atomic_checkpoint(&self, states: &HashMap<String, PrState>) -> Result<()> {
        let serialized = serde_json::to_string_pretty(states)?;
        let temp_path = self.file_path.with_extension(format!(
            "tmp.{}.{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        ));

        // Write temp file and sync_all
        let mut file = tokio::fs::File::create(&temp_path).await?;
        file.write_all(serialized.as_bytes()).await?;
        file.sync_all().await?;
        drop(file);

        // Atomic POSIX rename
        tokio::fs::rename(&temp_path, &self.file_path).await?;

        // Truncate WAL once checkpoint is atomically durable
        let _ = tokio::fs::remove_file(&self.wal_path).await;
        Ok(())
    }
}

fn chrono_iso_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod rollback_tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn clearing_the_reviewed_sha_allows_the_pr_to_be_retried() {
        let tmp = tempdir().unwrap();
        let sm = StateManager::load(tmp.path()).await.unwrap();

        sm.update_pr_state(
            "oyatie/anvil",
            42,
            "sha-abc".to_string(),
            Some("APPROVE".into()),
        )
        .await
        .unwrap();
        assert_eq!(
            sm.get_pr_state("oyatie/anvil", 42)
                .await
                .unwrap()
                .last_reviewed_head_sha,
            "sha-abc"
        );

        // A pipeline abort after stamping must not strand the PR: the guard in
        // the review pipeline skips any webhook whose head SHA matches the
        // stamp, so the stamp has to be released.
        sm.clear_reviewed_sha("oyatie/anvil", 42).await;
        assert_ne!(
            sm.get_pr_state("oyatie/anvil", 42)
                .await
                .unwrap()
                .last_reviewed_head_sha,
            "sha-abc",
            "the SHA must no longer match, or the retry is skipped"
        );
    }

    #[tokio::test]
    async fn the_rollback_survives_a_restart() {
        let tmp = tempdir().unwrap();
        {
            let sm = StateManager::load(tmp.path()).await.unwrap();
            sm.update_pr_state("oyatie/anvil", 7, "sha-xyz".to_string(), None)
                .await
                .unwrap();
            sm.clear_reviewed_sha("oyatie/anvil", 7).await;
        }
        let reloaded = StateManager::load(tmp.path()).await.unwrap();
        assert_ne!(
            reloaded
                .get_pr_state("oyatie/anvil", 7)
                .await
                .unwrap()
                .last_reviewed_head_sha,
            "sha-xyz"
        );
    }

    #[tokio::test]
    async fn clearing_an_unknown_pr_is_a_no_op() {
        let tmp = tempdir().unwrap();
        let sm = StateManager::load(tmp.path()).await.unwrap();
        sm.clear_reviewed_sha("oyatie/anvil", 999).await;
        assert!(sm.get_pr_state("oyatie/anvil", 999).await.is_none());
    }
}
