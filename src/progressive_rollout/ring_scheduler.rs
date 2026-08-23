use serde::{Deserialize, Serialize};

/// Azure region pairs, as published in the Azure reliability documentation.
///
/// Sequential updating across a pair is the Safe Deployment Practice this gate
/// is named for: "Azure strives to stagger any planned system updates across
/// region pairs", so a change goes to one half and then to the other, never to
/// both at once.
///
/// This table used to hold region codes from a different cloud, paired by the
/// rule "same prefix, adjacent numeric suffix". That is wrong twice over: those
/// are not Azure regions, and the cloud they came from publishes no region-pair
/// concept at all. The rule does not survive translation either — Azure pairs
/// East US with West US, not with East US 2, which pairs with Central US.
///
/// Deliberately partial. Azure publishes pairs that are asymmetric (West US 3
/// pairs to East US one-directionally) and a growing set of nonpaired regions
/// that use availability zones instead; neither is modelled here, and a region
/// this table does not name is treated as unpaired rather than guessed at.
const AZURE_REGION_PAIRS: &[(&str, &str)] = &[
    ("eastus", "westus"),
    ("eastus2", "centralus"),
    ("northeurope", "westeurope"),
    ("japaneast", "japanwest"),
    ("uksouth", "ukwest"),
    ("southeastasia", "eastasia"),
    ("koreacentral", "koreasouth"),
];

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeploymentRing {
    Ring0Canary,
    Ring1Dogfood,
    Ring2SingleCell,
    Ring3GlobalProd,
}

impl DeploymentRing {
    /// The next ring in progressive-exposure order. The broadest ring is
    /// terminal and returns itself.
    pub fn next(&self) -> DeploymentRing {
        match self {
            DeploymentRing::Ring0Canary => DeploymentRing::Ring1Dogfood,
            DeploymentRing::Ring1Dogfood => DeploymentRing::Ring2SingleCell,
            DeploymentRing::Ring2SingleCell => DeploymentRing::Ring3GlobalProd,
            DeploymentRing::Ring3GlobalProd => DeploymentRing::Ring3GlobalProd,
        }
    }
}

/// The step a rollout would advance to. Carries no health field: whether the
/// advance is permitted is a finding about observed rollout state, and putting
/// it here is what let four match arms answer it with the same literal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RingRolloutState {
    pub current_ring: DeploymentRing,
    pub target_ring: DeploymentRing,
    pub traffic_pct: u8,
}

/// A rollout policy a caller supplies.
///
/// There is deliberately no `Default`. A manifest describes a real service's
/// exposure schedule; one written here would be a schedule this crate invented
/// and then certified itself against — and it is exactly where the bake
/// minutes and the region strings used to live.
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

#[derive(Debug, Clone, Copy, Default)]
pub struct RingScheduler;

impl RingScheduler {
    pub fn new() -> Self {
        Self
    }

    /// The step the manifest schedules after `current`, or `None` when the
    /// manifest declares no config for that step.
    ///
    /// The traffic percentage is read from the manifest's own `RingConfig`.
    /// It used to be a literal beside each match arm, which meant two schedules
    /// existed and disagreed: the manifest declared one ring at a quarter of
    /// traffic while the scheduler published a fifth for the same ring.
    ///
    /// A target ring the manifest never declares returns `None`. It used to
    /// fall back to `unwrap_or_default()`, publishing a permitted advance to an
    /// unscheduled ring at `traffic_pct: 0` -- the absence of a rule read as
    /// compliance with it, which is the same inversion `validate_bake_window`
    /// below was rewritten to remove.
    pub fn compute_next_ring(
        &self,
        current: &DeploymentRing,
        manifest: &RolloutManifest,
    ) -> Option<RingRolloutState> {
        let target_ring = current.next();
        let traffic_pct = manifest
            .rings
            .iter()
            .find(|r| r.ring == target_ring)
            .map(|r| r.traffic_percentage)?;

        Some(RingRolloutState {
            current_ring: current.clone(),
            target_ring,
            traffic_pct,
        })
    }

    /// Whether the ring has baked for at least as long as its manifest demands.
    ///
    /// A ring the manifest does not declare returns false. This used to return
    /// true — the absence of a rule read as compliance with it, which is the
    /// same inversion this module was rewritten to remove, one level down.
    pub fn validate_bake_window(
        &self,
        ring: &DeploymentRing,
        elapsed_bake_minutes: u64,
        manifest: &RolloutManifest,
    ) -> bool {
        manifest
            .rings
            .iter()
            .find(|r| &r.ring == ring)
            .is_some_and(|config| elapsed_bake_minutes >= config.min_bake_minutes)
    }

    /// Whether the rollout is clear of both halves of any Azure region pair.
    ///
    /// Names are compared case-insensitively with spaces removed, so the
    /// display form and the ARM form of a region resolve to the same entry.
    pub fn validate_geo_paired_exclusion(&self, active_regions: &[String]) -> bool {
        let normalised: Vec<String> = active_regions
            .iter()
            .map(|r| r.to_lowercase().replace(' ', ""))
            .collect();

        !AZURE_REGION_PAIRS
            .iter()
            .any(|(a, b)| normalised.iter().any(|r| r == a) && normalised.iter().any(|r| r == b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> RolloutManifest {
        RolloutManifest {
            service_name: "svc".to_string(),
            geo_paired_exclusion_enabled: true,
            rings: vec![
                RingConfig {
                    ring: DeploymentRing::Ring0Canary,
                    traffic_percentage: 1,
                    min_bake_minutes: 60,
                    regions: vec!["eastus".to_string()],
                },
                RingConfig {
                    ring: DeploymentRing::Ring1Dogfood,
                    traffic_percentage: 5,
                    min_bake_minutes: 360,
                    regions: vec!["northeurope".to_string()],
                },
            ],
        }
    }

    #[test]
    fn the_advance_reads_its_traffic_percentage_from_the_manifest() {
        let next = RingScheduler::new()
            .compute_next_ring(&DeploymentRing::Ring0Canary, &manifest())
            .expect("the manifest declares Ring1Dogfood");
        assert_eq!(next.target_ring, DeploymentRing::Ring1Dogfood);
        assert_eq!(next.traffic_pct, 5);
    }

    #[test]
    fn a_ring_the_manifest_does_not_declare_is_not_a_step_it_schedules() {
        // Ring1Dogfood is the last ring `manifest()` declares, so the step out
        // of it has no config. An advance to it at `traffic_pct: 0` would be
        // the absence of a rule read as compliance with it.
        assert!(
            RingScheduler::new()
                .compute_next_ring(&DeploymentRing::Ring1Dogfood, &manifest())
                .is_none()
        );
    }

    #[test]
    fn every_ring_advances_to_the_next_one_in_exposure_order() {
        for (from, to) in [
            (DeploymentRing::Ring0Canary, DeploymentRing::Ring1Dogfood),
            (
                DeploymentRing::Ring1Dogfood,
                DeploymentRing::Ring2SingleCell,
            ),
            (
                DeploymentRing::Ring2SingleCell,
                DeploymentRing::Ring3GlobalProd,
            ),
            (
                DeploymentRing::Ring3GlobalProd,
                DeploymentRing::Ring3GlobalProd,
            ),
        ] {
            assert_eq!(from.next(), to, "wrong successor for {from:?}");
        }
    }

    #[test]
    fn the_broadest_ring_is_terminal() {
        let mut m = manifest();
        m.rings.push(RingConfig {
            ring: DeploymentRing::Ring3GlobalProd,
            traffic_percentage: 100,
            min_bake_minutes: 1440,
            regions: vec!["westus".to_string()],
        });
        let next = RingScheduler::new()
            .compute_next_ring(&DeploymentRing::Ring3GlobalProd, &m)
            .expect("the manifest declares Ring3GlobalProd");
        assert_eq!(next.target_ring, DeploymentRing::Ring3GlobalProd);
    }

    #[test]
    fn a_bake_window_is_enforced_and_an_undeclared_ring_has_none_to_satisfy() {
        let s = RingScheduler::new();
        assert!(!s.validate_bake_window(&DeploymentRing::Ring0Canary, 59, &manifest()));
        assert!(s.validate_bake_window(&DeploymentRing::Ring0Canary, 60, &manifest()));
        assert!(
            !s.validate_bake_window(&DeploymentRing::Ring2SingleCell, 9999, &manifest()),
            "a ring the manifest never declares has no bake window it can have satisfied"
        );
    }

    #[test]
    fn both_halves_of_an_azure_region_pair_may_not_take_the_rollout_together() {
        let s = RingScheduler::new();
        assert!(
            s.validate_geo_paired_exclusion(&["eastus".to_string(), "northeurope".to_string()])
        );
        assert!(!s.validate_geo_paired_exclusion(&["eastus".to_string(), "westus".to_string()]));
        assert!(
            !s.validate_geo_paired_exclusion(&["East US".to_string(), "westus".to_string()]),
            "the display form and the ARM form name the same region"
        );
    }
}
