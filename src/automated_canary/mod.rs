use serde::{Deserialize, Serialize};

pub mod statistical_engine;
pub use statistical_engine::{CanaryVerdict, MetricDistribution, StatisticalCanaryEngine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomatedCanaryReport {
    pub passed: bool,
    pub verdict: CanaryVerdict,
}

#[derive(Debug, Clone, Default)]
pub struct AutomatedCanaryAnalysis {
    engine: StatisticalCanaryEngine,
}

impl AutomatedCanaryAnalysis {
    pub fn new() -> Self {
        Self {
            engine: StatisticalCanaryEngine::new(),
        }
    }

    pub fn evaluate_canary(
        &self,
        distribution: &MetricDistribution,
    ) -> AutomatedCanaryReport {
        let verdict = self.engine.evaluate_canary_distributions(distribution);
        let passed = matches!(verdict, CanaryVerdict::Pass | CanaryVerdict::Marginal);

        AutomatedCanaryReport { passed, verdict }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_automated_canary_nominal() {
        let aca = AutomatedCanaryAnalysis::new();
        let dist = MetricDistribution {
            metric_name: "error_rate".to_string(),
            baseline_samples: vec![0.001],
            canary_samples: vec![0.001],
        };
        let report = aca.evaluate_canary(&dist);
        assert!(report.passed);
    }
}
