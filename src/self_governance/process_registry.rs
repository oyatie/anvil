use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessRecord {
    pub task_id: String,
    pub task_name: String,
    pub target_entity: String,
    pub max_sla: Duration,
    pub max_inactivity_window: Duration,
    pub estimated_cost_usd_limit: f64,
    pub current_tokens_used: usize,
}

struct InternalProcessState {
    record: ProcessRecord,
    started_at: Instant,
    last_vital_sign: Instant,
    is_active: bool,
}

#[derive(Clone, Default)]
pub struct ProcessRegistry {
    tasks: Arc<RwLock<HashMap<String, InternalProcessState>>>,
}

impl ProcessRegistry {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a newly spawned child task or LLM operation
    pub async fn register_task(&self, record: ProcessRecord) {
        let mut tasks = self.tasks.write().await;
        let now = Instant::now();
        info!(
            "📋 [Process Registry] Registered task '{}' ({}) on {} (SLA: {:.0}s, idle max: {:.0}s)",
            record.task_id,
            record.task_name,
            record.target_entity,
            record.max_sla.as_secs_f64(),
            record.max_inactivity_window.as_secs_f64()
        );
        tasks.insert(
            record.task_id.clone(),
            InternalProcessState {
                record,
                started_at: now,
                last_vital_sign: now,
                is_active: true,
            },
        );
    }

    /// Updates vital signs for an active task
    pub async fn record_vital_sign(&self, task_id: &str, tokens_increment: usize) {
        let mut tasks = self.tasks.write().await;
        if let Some(state) = tasks.get_mut(task_id) {
            state.last_vital_sign = Instant::now();
            state.record.current_tokens_used += tokens_increment;
        }
    }

    /// Marks a task as completed and removes it from active tracking
    pub async fn complete_task(&self, task_id: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(state) = tasks.remove(task_id) {
            let elapsed = state.started_at.elapsed();
            info!(
                "✅ [Process Registry] Task '{}' ({}) finished cleanly in {:.2}s (tokens used: {})",
                state.record.task_id,
                state.record.task_name,
                elapsed.as_secs_f64(),
                state.record.current_tokens_used
            );
        }
    }

    /// Identifies all stalled or SLA-breaching tasks for autonomous reaping
    pub async fn scan_stalled_tasks(&self) -> Vec<ProcessRecord> {
        let tasks = self.tasks.read().await;
        let now = Instant::now();
        let mut stalled = Vec::new();

        for state in tasks.values() {
            if !state.is_active {
                continue;
            }

            let idle_duration = now.duration_since(state.last_vital_sign);
            let total_duration = now.duration_since(state.started_at);

            if idle_duration > state.record.max_inactivity_window {
                warn!(
                    "🚨 [Process Registry] Task '{}' ({}) breached idle threshold: {:.1}s > {:.0}s",
                    state.record.task_id,
                    state.record.task_name,
                    idle_duration.as_secs_f64(),
                    state.record.max_inactivity_window.as_secs_f64()
                );
                stalled.push(state.record.clone());
            } else if total_duration > state.record.max_sla {
                error!(
                    "🚨 [Process Registry] Task '{}' ({}) breached global SLA limit: {:.1}s > {:.0}s",
                    state.record.task_id, state.record.task_name, total_duration.as_secs_f64(), state.record.max_sla.as_secs_f64()
                );
                stalled.push(state.record.clone());
            }
        }

        stalled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_registry_lifecycle() {
        let registry = ProcessRegistry::new();
        let record = ProcessRecord {
            task_id: "test-task-1".to_string(),
            task_name: "KaniVerification".to_string(),
            target_entity: "oyatie/anvil#1".to_string(),
            max_sla: Duration::from_secs(60),
            max_inactivity_window: Duration::from_millis(50),
            estimated_cost_usd_limit: 2.50,
            current_tokens_used: 0,
        };

        registry.register_task(record).await;
        assert_eq!(registry.scan_stalled_tasks().await.len(), 0);

        // Sleep beyond inactivity window
        tokio::time::sleep(Duration::from_millis(60)).await;
        let stalled = registry.scan_stalled_tasks().await;
        assert_eq!(stalled.len(), 1);
        assert_eq!(stalled[0].task_id, "test-task-1");

        registry.complete_task("test-task-1").await;
        assert_eq!(registry.scan_stalled_tasks().await.len(), 0);
    }
}
