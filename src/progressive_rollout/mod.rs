use serde::{Deserialize, Serialize};

pub mod ring_scheduler;
pub use ring_scheduler::{DeploymentRing, RingRolloutState, RingScheduler};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressiveRingReport {
    pub passed: bool,
    pub state: RingRolloutState,
}

#[derive(Debug, Clone, Default)]
pub struct ProgressiveRingOrchestrator {
    scheduler: RingScheduler,
}

impl ProgressiveRingOrchestrator {
    pub fn new() -> Self {
        Self {
            scheduler: RingScheduler::new(),
        }
    }

    pub fn evaluate_ring_rollout(
        &self,
        current_ring: &DeploymentRing,
        canary_healthy: bool,
    ) -> ProgressiveRingReport {
        let state = self
            .scheduler
            .compute_next_ring(current_ring, canary_healthy);
        ProgressiveRingReport {
            passed: state.is_healthy,
            state,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progressive_orchestrator_nominal() {
        let orch = ProgressiveRingOrchestrator::new();
        let rep = orch.evaluate_ring_rollout(&DeploymentRing::Ring0Canary, true);
        assert!(rep.passed);
    }
}
