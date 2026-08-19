#[derive(Clone, Debug)]
pub struct DependencyUpgradeCandidate {
    pub package_name: String,
    pub current_version: String,
    pub target_version: String,
    pub is_major_breaking: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TrainOrchestrator;

impl TrainOrchestrator {
    pub fn new() -> Self {
        Self
    }

    pub fn audit_upgrade_candidates(
        &self,
        candidates: &[DependencyUpgradeCandidate],
    ) -> (usize, usize) {
        let mut pending = 0;
        let mut breaking = 0;

        for c in candidates {
            if c.is_major_breaking {
                breaking += 1;
            } else {
                pending += 1;
            }
        }

        (pending, breaking)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audits_dependency_candidates() {
        let orchestrator = TrainOrchestrator::new();
        let candidates = vec![
            DependencyUpgradeCandidate {
                package_name: "serde".to_string(),
                current_version: "1.0.190".to_string(),
                target_version: "1.0.210".to_string(),
                is_major_breaking: false,
            },
            DependencyUpgradeCandidate {
                package_name: "tokio".to_string(),
                current_version: "1.0.0".to_string(),
                target_version: "2.0.0".to_string(),
                is_major_breaking: true,
            },
        ];
        let (pending, breaking) = orchestrator.audit_upgrade_candidates(&candidates);
        assert_eq!(pending, 1);
        assert_eq!(breaking, 1);
    }
}
