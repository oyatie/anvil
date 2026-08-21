pub mod dora_calculator;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

pub use dora_calculator::{DeploymentEvent, DoraCalculator, DoraMetricSnapshot, IncidentEvent};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetPrRecord {
    pub repo: String,
    pub pr_number: u64,
    pub title: String,
    pub author: String,
    pub head_sha: String,
    pub review_verdict: String,
    pub gates_passed: usize,
    pub gates_failed: usize,
    pub duration_seconds: u64,
    pub is_certified: bool,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateFailureRecord {
    pub repo: String,
    pub pr_number: u64,
    pub gate_name: String,
    pub failure_reason: String,
    pub timestamp: DateTime<Utc>,
}

/// One shape measurement of one repository at one revision, as measured —
/// counts derived from findings, never a stored verdict (I2).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShapeMeasurementRecord {
    pub repo: String,
    pub rev: String,
    pub spec_source: String,
    pub findings_total: usize,
    pub units_total: usize,
    pub units_conformant: usize,
    pub per_rule: std::collections::BTreeMap<String, usize>,
    pub blocking_regressions: usize,
    pub advisory_regressions: usize,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TelemetryStoreData {
    pub pr_history: Vec<FleetPrRecord>,
    pub gate_failures: Vec<GateFailureRecord>,
    pub deployments: Vec<DeploymentEvent>,
    pub incidents: Vec<IncidentEvent>,
    #[serde(default)]
    pub shape_measurements: Vec<ShapeMeasurementRecord>,
}

#[derive(Clone)]
pub struct TelemetryStore {
    storage_dir: PathBuf,
    data: Arc<RwLock<TelemetryStoreData>>,
}

impl TelemetryStore {
    pub async fn new<P: AsRef<Path>>(dir: P) -> Self {
        let storage_dir = dir.as_ref().to_path_buf();
        let _ = tokio::fs::create_dir_all(&storage_dir).await;

        let store = Self {
            storage_dir: storage_dir.clone(),
            data: Arc::new(RwLock::new(TelemetryStoreData::default())),
        };

        store.load_from_disk().await;
        store
    }

    pub async fn record_pr_event(&self, record: FleetPrRecord) {
        let mut d = self.data.write().await;
        if d.pr_history.len() >= 50_000 {
            d.pr_history.remove(0); // Evict oldest
        }
        d.pr_history.push(record);
        drop(d);
        let _ = self.persist_to_disk().await;
    }

    pub async fn record_gate_failure(&self, failure: GateFailureRecord) {
        let mut d = self.data.write().await;
        if d.gate_failures.len() >= 50_000 {
            d.gate_failures.remove(0);
        }
        d.gate_failures.push(failure);
        drop(d);
        let _ = self.persist_to_disk().await;
    }

    pub async fn record_shape_measurement(&self, rec: ShapeMeasurementRecord) {
        {
            let mut d = self.data.write().await;
            d.shape_measurements.push(rec);
            // Keep one year of hourly sweeps per repo at most; the journal is
            // a trend, not an archive.
            if d.shape_measurements.len() > 10_000 {
                let excess = d.shape_measurements.len() - 10_000;
                d.shape_measurements.drain(0..excess);
            }
        }
        let _ = self.persist_to_disk().await;
    }

    /// The latest measurement per repository.
    pub async fn latest_shape_measurements(&self) -> HashMap<String, ShapeMeasurementRecord> {
        let d = self.data.read().await;
        let mut out: HashMap<String, ShapeMeasurementRecord> = HashMap::new();
        for r in &d.shape_measurements {
            let newer = out
                .get(&r.repo)
                .is_none_or(|cur| cur.recorded_at <= r.recorded_at);
            if newer {
                out.insert(r.repo.clone(), r.clone());
            }
        }
        out
    }

    pub async fn record_deployment(&self, dep: DeploymentEvent) {
        let mut d = self.data.write().await;
        d.deployments.push(dep);
        drop(d);
        let _ = self.persist_to_disk().await;
    }

    pub async fn get_dora_metrics(&self, repo: &str, window_days: u32) -> DoraMetricSnapshot {
        let d = self.data.read().await;
        DoraCalculator::compute_dora(repo, &d.deployments, &d.incidents, window_days)
    }

    pub async fn get_gate_failure_heatmap(&self, repo: &str) -> HashMap<String, usize> {
        let d = self.data.read().await;
        let mut counts = HashMap::new();
        for f in d.gate_failures.iter().filter(|f| f.repo == repo) {
            *counts.entry(f.gate_name.clone()).or_insert(0) += 1;
        }
        counts
    }

    pub async fn get_recent_pr_history(&self, limit: usize) -> Vec<FleetPrRecord> {
        let d = self.data.read().await;
        d.pr_history.iter().rev().take(limit).cloned().collect()
    }

    async fn load_from_disk(&self) {
        let file_path = self.storage_dir.join("telemetry_journal.json");
        if file_path.exists()
            && let Ok(bytes) = tokio::fs::read(&file_path).await
            && let Ok(loaded) = serde_json::from_slice::<TelemetryStoreData>(&bytes)
        {
            let mut d = self.data.write().await;
            *d = loaded;
            info!(
                "📂 [Telemetry Store] Loaded {} PR records and {} gate failure entries from disk.",
                d.pr_history.len(),
                d.gate_failures.len()
            );
        }
    }

    async fn persist_to_disk(&self) -> Result<()> {
        let file_path = self.storage_dir.join("telemetry_journal.json");
        let d = self.data.read().await;
        let bytes = serde_json::to_vec_pretty(&*d)?;
        tokio::fs::write(&file_path, bytes).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_telemetry_store_persistence() {
        let unique_dir = format!(
            "anvil_telemetry_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let tmp = std::env::temp_dir().join(unique_dir);
        let store = TelemetryStore::new(&tmp).await;

        store
            .record_pr_event(FleetPrRecord {
                repo: "oyatie/anvil".to_string(),
                pr_number: 1,
                title: "feat: init".to_string(),
                author: "jason".to_string(),
                head_sha: "abc1234".to_string(),
                review_verdict: "APPROVE".to_string(),
                gates_passed: 70,
                gates_failed: 0,
                duration_seconds: 12,
                is_certified: true,
                recorded_at: Utc::now(),
            })
            .await;

        let recent = store.get_recent_pr_history(10).await;
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].pr_number, 1);
        let _ = tokio::fs::remove_dir_all(&tmp).await;
    }
}
