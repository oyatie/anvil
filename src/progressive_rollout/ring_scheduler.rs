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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutManifest {
    pub service_name: String,
    pub rings: Vec<RingConfig>,
    pub geo_paired_exclusion_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingConfig {
    pub ring: DeploymentRing,
    pub traffic_percentage: u8,
    pub min_bake_minutes: u64,
    pub regions: Vec<String>,
}

impl Default for RolloutManifest {
    fn default() -> Self {
        Self {
            service_name: "default-service".to_string(),
            geo_paired_exclusion_enabled: true,
            rings: vec![
                RingConfig {
                    ring: DeploymentRing::Ring0Canary,
                    traffic_percentage: 1,
                    min_bake_minutes: 60,
                    regions: vec!["us-east-1-canary".to_string()],
                },
                RingConfig {
                    ring: DeploymentRing::Ring1Dogfood,
                    traffic_percentage: 5,
                    min_bake_minutes: 360,
                    regions: vec!["us-east-1".to_string()],
                },
                RingConfig {
                    ring: DeploymentRing::Ring2SingleCell,
                    traffic_percentage: 25,
                    min_bake_minutes: 1440,
                    regions: vec!["us-west-2".to_string(), "eu-west-1".to_string()],
                },
                RingConfig {
                    ring: DeploymentRing::Ring3GlobalProd,
                    traffic_percentage: 100,
                    min_bake_minutes: 0,
                    regions: vec!["global".to_string()],
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RingScheduler;

impl RingScheduler {
    pub fn new() -> Self {
        Self
    }

    /// Determines next deployment ring progression step
    pub fn compute_next_ring(
        &self,
        current: &DeploymentRing,
        aca_passed: bool,
    ) -> RingRolloutState {
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

    /// Validates whether the active bake window duration satisfies the mandatory non-bypassable threshold
    pub fn validate_bake_window(
        &self,
        ring: &DeploymentRing,
        elapsed_bake_minutes: u64,
        manifest: &RolloutManifest,
    ) -> bool {
        if let Some(config) = manifest.rings.iter().find(|r| &r.ring == ring) {
            elapsed_bake_minutes >= config.min_bake_minutes
        } else {
            true
        }
    }

    /// Validates geo-paired exclusion locks (prevents simultaneous rollouts to paired regions)
    pub fn validate_geo_paired_exclusion(&self, active_regions: &[String]) -> bool {
        let paired_sets = [
            ("us-east-1", "us-east-2"),
            ("eu-west-1", "eu-central-1"),
            ("ap-northeast-1", "ap-northeast-2"),
        ];

        for (r1, r2) in paired_sets {
            let has_r1 = active_regions.iter().any(|r| r == r1);
            let has_r2 = active_regions.iter().any(|r| r == r2);
            if has_r1 && has_r2 {
                return false; // Geo-paired exclusion violation
            }
        }
        true
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

    #[test]
    fn test_bake_window_enforcement() {
        let scheduler = RingScheduler::new();
        let manifest = RolloutManifest::default();

        // Ring0Canary requires >= 60 minutes bake
        assert!(!scheduler.validate_bake_window(&DeploymentRing::Ring0Canary, 30, &manifest));
        assert!(scheduler.validate_bake_window(&DeploymentRing::Ring0Canary, 60, &manifest));
        assert!(scheduler.validate_bake_window(&DeploymentRing::Ring0Canary, 90, &manifest));
    }

    #[test]
    fn test_geo_paired_exclusion_detection() {
        let scheduler = RingScheduler::new();

        // Single region or unpaired regions pass
        let valid_regions = vec!["us-east-1".to_string(), "eu-west-1".to_string()];
        assert!(scheduler.validate_geo_paired_exclusion(&valid_regions));

        // Paired regions (us-east-1 and us-east-2) fail exclusion check
        let invalid_regions = vec!["us-east-1".to_string(), "us-east-2".to_string()];
        assert!(!scheduler.validate_geo_paired_exclusion(&invalid_regions));
    }
}
