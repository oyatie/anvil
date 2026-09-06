//! Progressive canary rollout — the gate that wrote the burn rate it judged.
//!
//! # What was here
//!
//! `evaluate_rollout_health` built a metrics snapshot four lines above the
//! comparison that read it, setting a five-minute error budget burn rate below
//! the ceiling it was then measured against. The ceiling and the latency bound
//! were literals in the same call. Nothing could move any of the three, so the
//! branch was decided at compile time, and the report published
//! "5m burn rate ... < ... threshold" as though it were a reading. The pull
//! request reached the gate only as a field in a log line.
//!
//! # What is here now
//!
//! Nothing is fabricated. This crate carries no HTTP client, drives no canary
//! deployment and holds no metrics endpoint, so there is no burn rate to read
//! and the gate reports `GateStatus::NotMeasured` naming that. Not `Passed`,
//! and not `Failed`, which would accuse every pull request in the fleet of an
//! error budget breach nobody can reproduce.
//!
//! # Distance from the oracle, beyond the missing endpoint
//!
//! [`CanaryCircuitBreaker`] is retained and still exported: it is honest
//! arithmetic over a caller-supplied reading and it is the seam a real query
//! plugs into. It is a *single-window* rule, which the Google SRE Workbook
//! walks through as Approach 4 and rejects for recall — the recommendation is
//! multiwindow, multi-burn-rate, pairing a long window against a short one and
//! expressing the threshold as a factor of the error budget rather than as a
//! bare ratio. Recorded in the fidelity registry rather than papered over.
//!
//! Argo Rollouts marks an unreachable provider `Error` and aborts; Flagger
//! refuses to start a canary whose provider is offline and counts a no-data
//! query as a failed check; Kayenta fails the canary outright once half its
//! metrics classify as no-data. None of them treats "could not measure" as
//! "measured healthy", which is what this gate used to do.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::report::GateStatus;

pub mod circuit_breaker;
pub use circuit_breaker::{CanaryCircuitBreaker, CanaryMetricsSnapshot, CircuitBreakerDecision};

/// Matches the `PreMergeCertificationReport` field name, so `unmeasured_gates`
/// names a gate a reader can look up in the fidelity registry.
const GATE_ID: &str = "canary_status";

/// What must exist before a burn rate can be read at all. Published verbatim as
/// the `NotMeasured` reason.
const MISSING_METRICS_SOURCE: &str = "no canary deployment is driven from here and no Prometheus or OpenTelemetry \
     metrics endpoint is reachable — this crate carries no HTTP client — so no \
     error budget burn rate and no tail latency were ever read";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryRolloutReport {
    pub status: GateStatus,
    /// The reading the verdict describes. `None` while `status` is
    /// `NotMeasured`: there is no reading for it to describe, and a zeroed
    /// snapshot would be indistinguishable from a healthy one.
    pub observed: Option<CanaryMetricsSnapshot>,
}

#[derive(Debug, Clone, Default)]
pub struct CanaryRolloutGuard {
    breaker: CanaryCircuitBreaker,
}

impl CanaryRolloutGuard {
    pub fn new() -> Self {
        Self {
            breaker: CanaryCircuitBreaker::new(),
        }
    }

    /// Judges a caller-supplied reading against caller-supplied ceilings.
    ///
    /// Both the reading and the bounds arrive from outside: a gate that states
    /// its own inputs is a gate that states its own verdict.
    pub fn evaluate_metrics_snapshot(
        &self,
        metrics: &CanaryMetricsSnapshot,
        max_burn_rate_ceiling: f64,
        max_p99_latency_ms: f64,
    ) -> CanaryRolloutReport {
        let decision =
            self.breaker
                .evaluate_metrics(metrics, max_burn_rate_ceiling, max_p99_latency_ms);

        let status = if decision.should_rollback {
            GateStatus::Failed(decision.reason.unwrap_or_else(|| {
                "canary circuit breaker tripped without naming a reason".to_string()
            }))
        } else {
            GateStatus::Passed
        };

        CanaryRolloutReport {
            status,
            observed: Some(metrics.clone()),
        }
    }

    /// The certification pipeline's entry point. See the module docs: there is
    /// no endpoint to query, so there is no reading to judge.
    pub fn evaluate_without_metrics_source(&self) -> CanaryRolloutReport {
        CanaryRolloutReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_METRICS_SOURCE.to_string(),
            },
            observed: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_absent_metrics_endpoint_is_not_a_healthy_canary() {
        let report = CanaryRolloutGuard::new().evaluate_without_metrics_source();
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(
            report.observed.is_none(),
            "nothing was read, so there is no reading to carry"
        );
    }

    #[test]
    fn a_supplied_reading_above_the_ceiling_trips_the_breaker() {
        let report = CanaryRolloutGuard::new().evaluate_metrics_snapshot(
            &CanaryMetricsSnapshot {
                step_traffic_percent: 5,
                p99_latency_ms: 30.0,
                error_rate_percent: 2.0,
                burn_rate_5m: 6.0,
            },
            3.0,
            150.0,
        );
        assert!(matches!(report.status, GateStatus::Failed(_)));
    }
}
