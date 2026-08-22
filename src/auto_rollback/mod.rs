use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "auto_rollback_status";

const MISSING_TELEMETRY_SOURCE: &str = "no canary error-rate or latency telemetry was read, so this \
     service's health is unknown for this pull request";

#[derive(Clone, Debug)]
pub struct AutoRollbackReport {
    pub status: GateStatus,
    pub passed: bool,
    pub rollback_triggered: bool,
    pub summary: String,
}

#[derive(Clone, Debug, Default)]
pub struct AutoRollbackPostmortemEngine;

impl AutoRollbackPostmortemEngine {
    pub fn new() -> Self {
        Self
    }

    /// The gate's answer when no telemetry was read.
    ///
    /// The pipeline supplied the literals `0.01` and `45.0`, which sit far
    /// below the degradation thresholds, so the rollback path was unreachable
    /// on every pull request.
    pub fn evaluate_without_telemetry_source(&self) -> AutoRollbackReport {
        AutoRollbackReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_TELEMETRY_SOURCE.to_string(),
            },
            passed: false,
            rollback_triggered: false,
            summary: MISSING_TELEMETRY_SOURCE.to_string(),
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
            let summary = format!(
                "Service {} degraded (Err: {:.1}%, P99: {:.1}ms). Triggered auto-rollback & generated postmortem.",
                service, error_rate_percentage, latency_p99_ms
            );

            AutoRollbackReport {
                status: GateStatus::Warning(summary.clone()),
                passed: false,
                rollback_triggered: true,
                summary,
            }
        } else {
            AutoRollbackReport {
                status: GateStatus::Passed,
                passed: true,
                rollback_triggered: false,
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

#[cfg(test)]
mod no_telemetry_source_tests {
    use super::*;

    /// The review pipeline called this with the literals `(repo, 0.01, 45.0)`
    /// on every PR. `is_degraded` is `error_rate > 5.0 || latency_p99 > 500.0`,
    /// so those two constants put the gate permanently on the healthy branch:
    /// the rollback path, the postmortem generator, and the failing status were
    /// all unreachable in production. The engine is correct; it was never given
    /// a real reading.
    #[test]
    fn absent_telemetry_is_reported_as_unmeasured_not_as_a_healthy_service() {
        let report = AutoRollbackPostmortemEngine::new().evaluate_without_telemetry_source();

        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!report.passed, "an unobserved service is not a healthy one");
        assert!(!report.rollback_triggered);
    }

    /// The measuring path must still fire on a degraded reading.
    #[test]
    fn a_degraded_reading_still_triggers_rollback() {
        let report =
            AutoRollbackPostmortemEngine::new().evaluate_health_and_rollback("svc", 9.5, 45.0);

        assert!(!report.passed, "9.5% error rate must not pass");
        assert!(report.rollback_triggered);
    }
}
