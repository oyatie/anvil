pub mod process_registry;
pub mod quota_enforcer;
pub mod resource_reaper;

pub use process_registry::{ProcessRecord, ProcessRegistry};
pub use quota_enforcer::{QuotaBudgetReport, QuotaEnforcer};
pub use resource_reaper::{AutonomousResourceReaper, GarbageCollectionReport};

use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

#[derive(Clone)]
pub struct SelfGovernor {
    pub registry: Arc<ProcessRegistry>,
    pub quota: Arc<QuotaEnforcer>,
    pub reaper: Arc<AutonomousResourceReaper>,
}

impl Default for SelfGovernor {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfGovernor {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(ProcessRegistry::new()),
            quota: Arc::new(QuotaEnforcer::default()),
            reaper: Arc::new(AutonomousResourceReaper::default()),
        }
    }

    /// Spawns the autonomous self-monitoring daemon in the background
    pub fn spawn_monitoring_daemon(&self) {
        let governor = self.clone();
        tokio::spawn(async move {
            info!("🛡️ [Self-Governor] Autonomous Self-Monitoring Daemon initialized (10s heartbeat cadence)");
            let mut interval = tokio::time::interval(Duration::from_secs(10));
            loop {
                interval.tick().await;

                // 1. Check for stalled or SLA-breaching tasks
                let stalled = governor.registry.scan_stalled_tasks().await;
                for task in stalled {
                    error!(
                        "🚨 [Self-Governor Action] Auto-reaping stalled task '{}' ({}) on {} to prevent quota drain!",
                        task.task_id, task.task_name, task.target_entity
                    );
                    governor.registry.complete_task(&task.task_id).await;
                }

                // 2. Perform periodic resource and worktree garbage collection
                let _ = governor.reaper.run_sweep(None).await;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_self_governor_init() {
        let governor = SelfGovernor::new();
        governor.spawn_monitoring_daemon();
        assert_eq!(governor.quota.current_spend_usd(), 0.0);
    }
}
