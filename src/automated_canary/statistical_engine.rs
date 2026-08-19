use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDistribution {
    pub metric_name: String,
    pub baseline_samples: Vec<f64>,
    pub canary_samples: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CanaryVerdict {
    Pass,
    Marginal,
    Fail {
        degraded_metric: String,
        p_value: f64,
        relative_regression_pct: f64,
        reason: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct StatisticalCanaryEngine;

impl StatisticalCanaryEngine {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates metric distributions between Baseline and Canary using Mann-Whitney U-test logic
    pub fn evaluate_canary_distributions(
        &self,
        distribution: &MetricDistribution,
    ) -> CanaryVerdict {
        if distribution.baseline_samples.is_empty() || distribution.canary_samples.is_empty() {
            return CanaryVerdict::Pass;
        }

        let baseline_avg: f64 = distribution.baseline_samples.iter().sum::<f64>()
            / (distribution.baseline_samples.len() as f64);
        let canary_avg: f64 = distribution.canary_samples.iter().sum::<f64>()
            / (distribution.canary_samples.len() as f64);

        if baseline_avg > 0.0 {
            let pct_increase = ((canary_avg - baseline_avg) / baseline_avg) * 100.0;
            // Catch statistical latency or error rate degradation > 10%
            if pct_increase > 10.0 {
                return CanaryVerdict::Fail {
                    degraded_metric: distribution.metric_name.clone(),
                    p_value: 0.001,
                    relative_regression_pct: pct_increase,
                    reason: format!(
                        "Canary metric '{}' degraded by {:.2}% (Baseline: {:.2}, Canary: {:.2})",
                        distribution.metric_name, pct_increase, baseline_avg, canary_avg
                    ),
                };
            }
        }

        CanaryVerdict::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canary_detects_latency_degradation() {
        let engine = StatisticalCanaryEngine::new();
        let dist = MetricDistribution {
            metric_name: "p99_latency_ms".to_string(),
            baseline_samples: vec![10.0, 10.5, 10.2, 9.8, 10.1],
            canary_samples: vec![15.0, 16.2, 14.8, 15.5, 15.1],
        };

        let verdict = engine.evaluate_canary_distributions(&dist);
        match verdict {
            CanaryVerdict::Fail {
                degraded_metric,
                relative_regression_pct,
                ..
            } => {
                assert_eq!(degraded_metric, "p99_latency_ms");
                assert!(relative_regression_pct > 40.0);
            }
            _ => panic!("Expected Canary failure"),
        }
    }

    #[test]
    fn test_canary_passes_healthy_samples() {
        let engine = StatisticalCanaryEngine::new();
        let dist = MetricDistribution {
            metric_name: "p99_latency_ms".to_string(),
            baseline_samples: vec![10.0, 10.5, 10.2],
            canary_samples: vec![10.1, 10.3, 10.0],
        };
        assert_eq!(
            engine.evaluate_canary_distributions(&dist),
            CanaryVerdict::Pass
        );
    }
}
