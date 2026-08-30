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
    /// The head a run of the review pipeline reached the END of, whatever it
    /// decided when it got there.
    ///
    /// `last_reviewed_head_sha` is stamped early, before the gate corpus, the
    /// attestation receipt, the scorecard and the enlist decision -- which is
    /// most of the pipeline's wall clock. `last_certified_head_sha` is written
    /// only for a head the corpus certified AND the merge queue took, so it
    /// cannot stand in for "finished" either: a pull request the pipeline
    /// deliberately halted carries the stamp and no certification, and so does
    /// one whose process was killed a second after the stamp. Nothing durable
    /// separated those two, and they need opposite treatment.
    ///
    /// `serde(default)` for the reason the field above gives, plus one more:
    /// on the first boot after this field lands every open pull request reads
    /// as not-completed, so each is reviewed once more and then recorded. That
    /// pass is the point -- it is what releases the pull requests an earlier
    /// restart froze.
    #[serde(default)]
    pub last_completed_head_sha: Option<String>,
}

impl PrState {
    /// Whether a review run was stamped for `head_sha` and never finished it.
    ///
    /// Every exit after the stamp either rolls the stamp back through
    /// `clear_reviewed_sha` -- which
    /// `tests/a_stamped_pull_request_is_never_stranded_test.rs` enforces for
    /// the whole window -- or reaches the completion write at the tail of the
    /// pipeline. So a stamp with neither behind it is exactly "the process
    /// died inside the window", which no in-process rollback can cover:
    /// `main.rs` ends the daemon with `std::process::exit(0)` and every review
    /// pipeline is a detached task.
    ///
    /// This is the narrow predicate the dispatch guard needs. A run that
    /// reached the end of the pipeline and halted there is deliberately NOT
    /// stranded: re-reviewing it spends a model turn and posts a second review
    /// for a head that already has one.
    pub fn is_stranded_at(&self, head_sha: &str) -> bool {
        !self.last_reviewed_head_sha.is_empty()
            && self.last_reviewed_head_sha == head_sha
            && self.last_completed_head_sha.as_deref() != Some(head_sha)
    }
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
        let (states, cleared) = {
            let mut states = self.states.write().await;
            match states.get_mut(&key) {
                Some(entry) => entry.last_reviewed_head_sha.clear(),
                None => return,
            }
            let cleared = states.get(&key).cloned();
            (states.clone(), cleared)
        };
        // The log before the checkpoint, as `update_pr_state` does: the
        // checkpoint is a whole-file rewrite, the log is append-only, and a
        // crash between them is recovered from the log.
        if let Some(entry) = cleared {
            let wal = WalEntry {
                timestamp: chrono_iso_now(),
                key: key.clone(),
                state: entry,
            };
            if let Err(e) = self.append_wal(&wal).await {
                tracing::warn!(
                    "Could not log the reviewed-SHA rollback for {}#{}: {}",
                    repo,
                    pr_number,
                    e
                );
            }
        }
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

    /// Count one fixer run against this pull request, at this head.
    ///
    /// Recorded BEFORE the fixer runs, deliberately. A fixer that panics or
    /// times out must still consume its attempt, or a crashing fixer is an
    /// unbounded loop: the bound only holds if it counts tries rather than
    /// successes.
    pub async fn record_auto_fix_attempt(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
    ) -> Result<()> {
        let key = Self::key(repo, pr_number);
        let mut states = self.states.write().await;
        let entry = states.entry(key.clone()).or_default();
        entry.auto_fix_attempts = entry.auto_fix_attempts.saturating_add(1);
        entry.last_auto_fixed_head_sha = Some(head_sha.to_string());

        // Through the WAL, not just memory. A count that lives only in the
        // process resets on restart, and a bound that resets is not a bound --
        // the daemon would resume rewriting a pull request it had already
        // given up on three times.
        let wal_entry = WalEntry {
            timestamp: chrono_iso_now(),
            key: key.clone(),
            state: entry.clone(),
        };
        self.append_wal(&wal_entry).await?;
        self.atomic_checkpoint(&states).await?;
        Ok(())
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

    /// Record that the review pipeline ran to the end for `head_sha`.
    ///
    /// Written for every head the pipeline finished, certified or not, and
    /// written last so that it means "nothing is still owed for this head".
    /// [`PrState::is_stranded_at`] is its only reader, and it reads the
    /// absence: a head stamped as reviewed with no completion behind it is one
    /// a killed process left half-done, and is the only kind of head a later
    /// dispatch may review again without being asked to.
    pub async fn record_pipeline_completion(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
    ) -> Result<()> {
        let key = Self::key(repo, pr_number);
        let mut states = self.states.write().await;
        let entry = states.entry(key.clone()).or_default();
        entry.last_completed_head_sha = Some(head_sha.to_string());

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
