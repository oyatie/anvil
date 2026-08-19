pub mod postmortem_generator;

use postmortem_generator::{PostmortemBundle, PostmortemGenerator};

#[derive(Clone, Debug)]
pub struct AutoRollbackReport {
    pub passed: bool,
    pub rollback_triggered: bool,
    pub postmortem: Option<PostmortemBundle>,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct AutoRollbackPostmortemEngine {
    generator: PostmortemGenerator,
}

impl Default for AutoRollbackPostmortemEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoRollbackPostmortemEngine {
    pub fn new() -> Self {
        Self {
            generator: PostmortemGenerator::new(),
        }
    }

    pub fn evaluate_health_and_rollback(
        &self,
        service: &str,
        error_rate_percentage: f64,
        latency_p99_ms: f64,
    ) -> AutoRollbackReport {
        let is_degraded = error_rate_percentage > 5.0 || latency_p99_ms > 500.0;

        if is_degraded {
            let bundle = self.generator.generate_postmortem(
                service,
                "Canary error budget burn rate exceeded critical threshold (>5%)",
                error_rate_percentage,
                latency_p99_ms,
            );

            AutoRollbackReport {
                passed: false,
                rollback_triggered: true,
                postmortem: Some(bundle),
                summary: format!(
                    "Service {} degraded (Err: {:.1}%, P99: {:.1}ms). Triggered auto-rollback & generated postmortem.",
                    service, error_rate_percentage, latency_p99_ms
                ),
            }
        } else {
            AutoRollbackReport {
                passed: true,
                rollback_triggered: false,
                postmortem: None,
                summary: format!(
                    "Service {} healthy (Err: {:.1}%, P99: {:.1}ms). Zero rollback necessary.",
                    service, error_rate_percentage, latency_p99_ms
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_rollback_nominal() {
        let engine = AutoRollbackPostmortemEngine::new();
        let report = engine.evaluate_health_and_rollback("auth-service", 0.01, 45.0);
        assert!(report.passed);
        assert!(!report.rollback_triggered);
    }
}
