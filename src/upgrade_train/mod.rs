pub mod train_orchestrator;

pub use train_orchestrator::{DependencyUpgradeCandidate, TrainOrchestrator};

#[derive(Clone, Debug)]
pub struct UpgradeTrainReport {
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
