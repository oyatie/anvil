use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeploymentRing {
    Ring0Canary,
    Ring1Dogfood,
    Ring2SingleCell,
    Ring3GlobalProd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingRolloutState {
    pub current_ring: DeploymentRing,
    pub target_ring: DeploymentRing,
    pub traffic_pct: u8,
    pub is_healthy: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RingScheduler;

impl RingScheduler {
    pub fn new() -> Self {
        Self
    }

    /// Determines next deployment ring progression step
    pub fn compute_next_ring(&self, current: &DeploymentRing, aca_passed: bool) -> RingRolloutState {
        if !aca_passed {
            return RingRolloutState {
                current_ring: current.clone(),
                target_ring: DeploymentRing::Ring0Canary,
                traffic_pct: 0,
                is_healthy: false,
            };
        }

        match current {
            DeploymentRing::Ring0Canary => RingRolloutState {
                current_ring: DeploymentRing::Ring0Canary,
                target_ring: DeploymentRing::Ring1Dogfood,
                traffic_pct: 5,
                is_healthy: true,
            },
            DeploymentRing::Ring1Dogfood => RingRolloutState {
                current_ring: DeploymentRing::Ring1Dogfood,
                target_ring: DeploymentRing::Ring2SingleCell,
                traffic_pct: 20,
                is_healthy: true,
            },
            DeploymentRing::Ring2SingleCell => RingRolloutState {
                current_ring: DeploymentRing::Ring2SingleCell,
                target_ring: DeploymentRing::Ring3GlobalProd,
                traffic_pct: 100,
                is_healthy: true,
            },
            DeploymentRing::Ring3GlobalProd => RingRolloutState {
                current_ring: DeploymentRing::Ring3GlobalProd,
                target_ring: DeploymentRing::Ring3GlobalProd,
                traffic_pct: 100,
                is_healthy: true,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_progression_advances() {
        let scheduler = RingScheduler::new();
        let next = scheduler.compute_next_ring(&DeploymentRing::Ring0Canary, true);
        assert_eq!(next.target_ring, DeploymentRing::Ring1Dogfood);
        assert_eq!(next.traffic_pct, 5);
        assert!(next.is_healthy);
    }

    #[test]
    fn test_ring_progression_aborts_on_failure() {
        let scheduler = RingScheduler::new();
        let next = scheduler.compute_next_ring(&DeploymentRing::Ring2SingleCell, false);
        assert_eq!(next.target_ring, DeploymentRing::Ring0Canary);
        assert_eq!(next.traffic_pct, 0);
        assert!(!next.is_healthy);
    }
}
