pub mod train_orchestrator;

pub use train_orchestrator::{DependencyUpgradeCandidate, TrainOrchestrator};

use crate::pre_merge_guard::GateStatus;

const GATE_ID: &str = "upgrade_train_status";

const MISSING_DEPENDENCY_SOURCE: &str = "no dependency manifest or advisory feed was read, so no upgrade \
     candidates were audited for this pull request";

#[derive(Clone, Debug)]
pub struct UpgradeTrainReport {
    pub status: GateStatus,
    pub passed: bool,
    pub pending_upgrades_available: usize,
    pub breaking_major_upgrades: usize,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct ProactiveUpgradeTrain {
    orchestrator: TrainOrchestrator,
}

impl Default for ProactiveUpgradeTrain {
    fn default() -> Self {
        Self::new()
    }
}

impl ProactiveUpgradeTrain {
    pub fn new() -> Self {
        Self {
            orchestrator: TrainOrchestrator::new(),
        }
    }

    /// The gate's answer when no upgrade candidates were supplied.
    ///
    /// The pipeline passed `&[]` on every PR, and `breaking == 0` is trivially
    /// true of no candidates, so the gate certified an upgrade train it had
    /// never looked at.
    pub fn evaluate_without_dependency_source(&self) -> UpgradeTrainReport {
        UpgradeTrainReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_DEPENDENCY_SOURCE.to_string(),
            },
            passed: false,
            pending_upgrades_available: 0,
            breaking_major_upgrades: 0,
            summary: MISSING_DEPENDENCY_SOURCE.to_string(),
        }
    }

    pub fn evaluate_upgrade_train(
        &self,
        candidates: &[DependencyUpgradeCandidate],
    ) -> UpgradeTrainReport {
        let (pending, breaking) = self.orchestrator.audit_upgrade_candidates(candidates);
        let passed = breaking == 0;

        let summary = if passed {
            format!(
                "Proactive Dependency Upgrade Train: {} non-breaking upgrades certified for autonomous PR scheduling.",
                pending
            )
        } else {
            format!(
                "Proactive Dependency Upgrade Train: {} breaking major upgrades flagged for migration planning.",
                breaking
            )
        };

        UpgradeTrainReport {
            status: if passed {
                GateStatus::Passed
            } else {
                GateStatus::Failed(summary.clone())
            },
            passed,
            pending_upgrades_available: pending,
            breaking_major_upgrades: breaking,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_train_nominal() {
        let train = ProactiveUpgradeTrain::new();
        let report = train.evaluate_upgrade_train(&[]);
        assert!(report.passed);
    }
}

#[cfg(test)]
mod no_dependency_source_tests {
    use super::*;

    /// `breaking == 0` is trivially true of an empty candidate list, and the
    /// pipeline passed `&[]` on every PR -- so the gate certified an upgrade
    /// train it had never read.
    #[test]
    fn absent_candidates_are_reported_as_unmeasured_not_as_a_clean_train() {
        let report = ProactiveUpgradeTrain::new().evaluate_without_dependency_source();

        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!report.passed, "an unread dependency set is not a pass");
        assert_eq!(report.pending_upgrades_available, 0);
    }

    /// The measuring path must still fail on a breaking upgrade.
    #[test]
    fn a_breaking_major_upgrade_still_fails_the_train() {
        let report =
            ProactiveUpgradeTrain::new().evaluate_upgrade_train(&[DependencyUpgradeCandidate {
                package_name: "serde".to_string(),
                current_version: "1.0.0".to_string(),
                target_version: "2.0.0".to_string(),
                is_major_breaking: true,
            }]);

        assert!(!report.passed, "a breaking major upgrade must not pass");
        assert!(matches!(report.status, GateStatus::Failed(_)));
    }
}
