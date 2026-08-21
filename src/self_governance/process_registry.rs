use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::AbortHandle;
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
    pub os_pid: Option<u32>,
}

struct InternalProcessState {
    record: ProcessRecord,
    started_at: Instant,
    last_vital_sign: Instant,
    is_active: bool,
    abort_handle: Option<AbortHandle>,
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
        self.register_task_with_abort(record, None).await;
    }

    /// Registers a task with an abort handle for guaranteed resource reclamation
    pub async fn register_task_with_abort(
        &self,
        record: ProcessRecord,
        abort_handle: Option<AbortHandle>,
    ) {
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
                abort_handle,
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

    /// Explicitly cancels and aborts a running task and terminates any OS child process
    pub async fn cancel_and_reap_task(&self, task_id: &str, reason: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(mut state) = tasks.remove(task_id) {
            state.is_active = false;
            if let Some(abort) = state.abort_handle.take() {
                info!(
                    "🛑 [Process Registry] Aborting async task handle for '{}' (reason: {})",
                    task_id, reason
                );
                abort.abort();
            }
            if let Some(pid) = state.record.os_pid {
                warn!(
                    "🛑 [Process Registry] Killing orphaned OS process PID {} for '{}'",
                    pid, task_id
                );
                let mut kill_cmd = tokio::process::Command::new("kill");
                kill_cmd.args(["-9", &pid.to_string()]);
                let _ = crate::exec::run_bounded(
                    kill_cmd,
                    crate::exec::ExecClass::Quick,
                    "kill -9 (orphaned process)",
                )
                .await;
            }
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
                    "⚠️ [Process Registry] Task '{}' exceeded sliding idle window ({:.1}s > {:.1}s)",
                    state.record.task_id,
                    idle_duration.as_secs_f64(),
                    state.record.max_inactivity_window.as_secs_f64()
                );
                stalled.push(state.record.clone());
            } else if total_duration > state.record.max_sla {
                error!(
                    "🚨 [Process Registry] Task '{}' breached hard SLA ceiling ({:.1}s > {:.1}s)",
                    state.record.task_id,
                    total_duration.as_secs_f64(),
                    state.record.max_sla.as_secs_f64()
                );
                stalled.push(state.record.clone());
            }
        }

        stalled
    }

    /// Returns the number of currently active registered tasks
    pub async fn active_task_count(&self) -> usize {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|s| s.is_active).count()
    }
}
