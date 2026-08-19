use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PrState {
    pub last_reviewed_head_sha: String,
    pub last_reviewed_at: String,
    pub review_count: u32,
    pub last_review_verdict: Option<String>,
    pub last_certified_head_sha: Option<String>,
    pub is_enlisted_in_merge_queue: bool,
}

#[derive(Debug)]
pub struct StateManager {
    file_path: PathBuf,
    states: RwLock<HashMap<String, PrState>>,
}

impl StateManager {
    pub async fn load(data_dir: &Path) -> Result<Self> {
        tokio::fs::create_dir_all(data_dir)
            .await
            .context("Failed to create data directory")?;

        let file_path = data_dir.join("pr_states.json");
        let states = if file_path.exists() {
            let content = tokio::fs::read_to_string(&file_path)
                .await
                .context("Failed to read pr_states.json")?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Ok(Self {
            file_path,
            states: RwLock::new(states),
        })
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

        let entry = states.entry(key).or_default();
        entry.last_reviewed_head_sha = head_sha;
        entry.last_reviewed_at = chrono_iso_now();
        entry.review_count += 1;
        entry.last_review_verdict = verdict;

        let updated = entry.clone();

        // Persist to disk
        let serialized = serde_json::to_string_pretty(&*states)?;
        tokio::fs::write(&self.file_path, serialized)
            .await
            .context("Failed to save pr_states.json")?;

        Ok(updated)
    }

    pub async fn mark_certified_and_enlisted(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: String,
    ) -> Result<PrState> {
        let key = Self::key(repo, pr_number);
        let mut states = self.states.write().await;

        let entry = states.entry(key).or_default();
        entry.last_certified_head_sha = Some(head_sha);
        entry.is_enlisted_in_merge_queue = true;

        let updated = entry.clone();

        let serialized = serde_json::to_string_pretty(&*states)?;
        tokio::fs::write(&self.file_path, serialized)
            .await
            .context("Failed to save pr_states.json")?;

        Ok(updated)
    }
}

fn chrono_iso_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{}", duration.as_secs(), duration.subsec_millis())
}
