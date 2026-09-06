//! Progressive rollout rings — the gate whose health verdict was a literal
//! threaded through three calls.
//!
//! # What was here
//!
//! The certification pipeline passed `aca_report.status.is_acceptable()`, which
//! is `true` for `NotMeasured`; the orchestrator passed it on as `aca_passed`;
//! and `compute_next_ring` answered every one of its four match arms with the
//! same struct literal `true`. So the constant survived three hops and no
//! literal ever appeared in an argument list, which is why a scan for
//! fabricated arguments could not see it.
//!
//! Underneath that, the two functions in this module that check something real
//! — the bake window and the region-pair exclusion — had **zero production call
//! sites**. They were written, unit-tested, and never reached. The Azure Safe
//! Deployment Practices content was present and unwired.
//!
//! # What is here now
//!
//! Both validators are wired: [`ProgressiveRingOrchestrator::evaluate_ring_advance`]
//! is the only path to a ring advance and it runs both. That is the measuring
//! path, and it can fail — a short bake or a rollout straddling a region pair
//! holds the ring.
//!
//! What it needs is rollout state: how long the artefact currently occupying
//! the ring has been baking, and which regions are taking it right now. Azure
//! defines bake time as "the amount of time a deployment is allowed to run"
//! before expanding, which is meaningless for a change that has never been
//! deployed. Neither value is in a pull request diff, and nothing here talks to
//! a cloud control plane, so the certification pipeline calls
//! [`ProgressiveRingOrchestrator::evaluate_without_rollout_state`] and the gate
//! reports `GateStatus::NotMeasured`.
//!
//! # Why not simply inherit the canary's verdict
//!
//! Because `NotMeasured.is_acceptable()` is true, and Azure's rule runs the
//! other way: "Deployments must pass health checks before each phase of
//! progressive exposure can begin". An affirmative health signal is required to
//! advance. The absence of a negative one is not one — the Well-Architected
//! guidance says so directly, warning that "a lack of user-reported issues and
//! negative health signals aren't hiding an issue". Inheriting acceptability
//! from an unqueried canary turned exactly that absence into an advance.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::report::GateStatus;

pub mod ring_scheduler;
pub use ring_scheduler::{
    DeploymentRing, RingConfig, RingRolloutState, RingScheduler, RolloutManifest,
};

/// Matches the `PreMergeCertificationReport` field name.
const GATE_ID: &str = "progressive_ring_status";

/// What must exist before a ring advance can be judged at all.
const MISSING_ROLLOUT_STATE: &str = "nothing here deploys a ring and no cloud control plane is reachable, so the elapsed \
     bake time of whatever occupies the ring and the set of regions currently taking the \
     rollout are both unknown; a pull request diff carries neither";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressiveRingReport {
    pub status: GateStatus,
    /// The step the rollout would advance to. `None` unless the advance was
    /// judged and permitted — a held or unmeasured ring advances to nothing.
    pub state: Option<RingRolloutState>,
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

    /// Judges a caller-supplied rollout state against a caller-supplied
    /// manifest, and returns the step it would advance to if both checks hold.
    pub fn evaluate_ring_advance(
        &self,
        current_ring: &DeploymentRing,
        elapsed_bake_minutes: u64,
        active_regions: &[String],
        manifest: &RolloutManifest,
    ) -> ProgressiveRingReport {
        let mut holds: Vec<String> = Vec::new();

        let next = self.scheduler.compute_next_ring(current_ring, manifest);
        if next.is_none() {
            holds.push(format!(
                "`{}` declares no config for {:?}, the ring {current_ring:?} would advance to; \
                 an undeclared ring has no traffic percentage to advance at",
                manifest.service_name,
                current_ring.next()
            ));
        }

        if !self
            .scheduler
            .validate_bake_window(current_ring, elapsed_bake_minutes, manifest)
        {
            holds.push(format!(
                "{current_ring:?} has baked for {elapsed_bake_minutes} minutes, short of the \
                 minimum `{}` declares for it",
                manifest.service_name
            ));
        }

        if manifest.geo_paired_exclusion_enabled
            && !self.scheduler.validate_geo_paired_exclusion(active_regions)
        {
            holds.push(format!(
                "the rollout is active in both halves of an Azure region pair ({}); paired \
                 regions are updated sequentially, never together",
                active_regions.join(", ")
            ));
        }

        if holds.is_empty() {
            ProgressiveRingReport {
                status: GateStatus::Passed,
                state: next,
            }
        } else {
            ProgressiveRingReport {
                status: GateStatus::Failed(holds.join("; ")),
                state: None,
            }
        }
    }

    /// The certification pipeline's entry point. See the module docs: no
    /// rollout state exists to judge.
    pub fn evaluate_without_rollout_state(&self) -> ProgressiveRingReport {
        ProgressiveRingReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_ROLLOUT_STATE.to_string(),
            },
            state: None,
        }
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
                // The ring Ring0Canary advances INTO. Without it every advance
                // below would be held for an undeclared target, which is the
                // subject of its own test rather than a precondition of these.
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
    fn a_manifest_that_does_not_declare_the_target_ring_holds_the_advance() {
        let mut only_canary = manifest();
        only_canary
            .rings
            .retain(|r| r.ring == DeploymentRing::Ring0Canary);
        let report = ProgressiveRingOrchestrator::new().evaluate_ring_advance(
            &DeploymentRing::Ring0Canary,
            60,
            &["eastus".to_string()],
            &only_canary,
        );
        match report.status {
            GateStatus::Failed(ref why) => assert!(
                why.contains("declares no config for Ring1Dogfood"),
                "the hold must name the ring the manifest is missing, got: {why}"
            ),
            other => panic!(
                "a bake-complete ring with an undeclared successor must not advance, got {other:?}"
            ),
        }
        assert!(
            report.state.is_none(),
            "a held ring advances to nothing, least of all to an unscheduled ring at 0% traffic"
        );
    }

    #[test]
    fn absent_rollout_state_is_not_a_healthy_ring() {
        let report = ProgressiveRingOrchestrator::new().evaluate_without_rollout_state();
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(report.state.is_none());
    }

    #[test]
    fn a_manifest_may_switch_the_region_pair_check_off() {
        let orch = ProgressiveRingOrchestrator::new();
        let paired = ["eastus".to_string(), "westus".to_string()];

        let mut off = manifest();
        off.geo_paired_exclusion_enabled = false;
        assert!(
            matches!(
                orch.evaluate_ring_advance(&DeploymentRing::Ring0Canary, 60, &paired, &off)
                    .status,
                GateStatus::Passed
            ),
            "with the exclusion disabled the pair is not a hold"
        );
        assert!(
            matches!(
                orch.evaluate_ring_advance(&DeploymentRing::Ring0Canary, 60, &paired, &manifest())
                    .status,
                GateStatus::Failed(_)
            ),
            "with it enabled the same regions must hold the ring"
        );
    }

    #[test]
    fn a_short_bake_holds_the_ring_and_a_completed_one_advances_it() {
        let orch = ProgressiveRingOrchestrator::new();
        let regions = ["eastus".to_string()];
        assert!(matches!(
            orch.evaluate_ring_advance(&DeploymentRing::Ring0Canary, 59, &regions, &manifest())
                .status,
            GateStatus::Failed(_)
        ));
        let advanced =
            orch.evaluate_ring_advance(&DeploymentRing::Ring0Canary, 60, &regions, &manifest());
        assert!(matches!(advanced.status, GateStatus::Passed));
        assert!(advanced.state.is_some());
    }
}
