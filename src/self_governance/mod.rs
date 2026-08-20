pub mod account_pool;
pub mod deathloop_detector;
pub mod process_registry;
pub mod quota_enforcer;
pub mod resource_reaper;
pub mod worktree_lease;

pub use account_pool::{
    AccountPoolManager, AccountQuotaView, AddAccountPayload, AuthType, DrainAccountPayload,
    ManagedAccount, UsageRecord,
};
pub use deathloop_detector::{DeathloopDetector, DeathloopVerdict};
pub use process_registry::{ProcessRecord, ProcessRegistry};
pub use quota_enforcer::{QuotaBudgetReport, QuotaEnforcer};
pub use resource_reaper::{AutonomousResourceReaper, GarbageCollectionReport};
pub use worktree_lease::{LeaseStore, LeaseVerdict, WorktreeLease};

use std::sync::Arc;
use std::time::Duration;
use tracing::{error, info};

#[derive(Clone)]
pub struct SelfGovernor {
    pub registry: Arc<ProcessRegistry>,
    pub quota: Arc<QuotaEnforcer>,
    pub reaper: Arc<AutonomousResourceReaper>,
    pub deathloop: Arc<DeathloopDetector>,
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
            deathloop: Arc::new(DeathloopDetector::default()),
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

                // 2. Perform periodic resource garbage collection.
                //
                // `run_sweep` takes no repo argument: the previous signature
                // was `run_sweep(Option<&Path>)` and this call site passed
                // `None`, which skipped the entire worktree path ~8,640 times
                // a day. The set of worktrees under consideration now comes
                // from a lease store instead.
                //
                // The governor's reaper is built with
                // `AutonomousResourceReaper::default()`, which carries NO lease
                // store, so worktree reclaim stays OFF on this cadence. Arming
                // it means constructing the reaper with `with_lease_store` and
                // is a deliberate, separate decision.
                let _ = governor.reaper.run_sweep().await;
            }
        });
    }
}
