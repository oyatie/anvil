//! Automated canary analysis — the gate that compared numbers nobody measured.
//!
//! # What was here
//!
//! `AutomatedCanaryReport` carried a single `passed: bool`, and the review
//! pipeline wrote the distribution it was then asked to judge:
//! `baseline_samples: vec![10.0, 10.2, 9.9]` against
//! `canary_samples: vec![10.1, 10.3, 10.0]`. Those two means differ by about
//! 1%, fixed at compile time, and the engine only fails above 10% — so the gate
//! was unfailable and its green described a literal in the caller rather than
//! the pull request.
//!
//! Worse, the engine returned `CanaryVerdict::Pass` when both sample vectors
//! were empty, so "no data" and "no regression" were the same value: absent
//! evidence published as a pass, the inversion I1 forbids.
//!
//! # What is here now
//!
//! No distribution is fabricated. With no canary deployment and no metrics
//! endpoint there are no samples, so the gate reports
//! `GateStatus::NotMeasured` naming the missing source — not `Passed`, and not
//! `Failed`, which would accuse every pull request in the fleet of a latency
//! regression nobody can reproduce.
//!
//! `StatisticalCanaryEngine` is retained and still exported: it is honest
//! arithmetic over a caller-supplied distribution and it is the seam a real
//! metrics source plugs into. Only the caller that supplied itself is deleted.
//! The engine now reports `CanaryVerdict::NotMeasured` for an empty
//! distribution instead of `Pass`.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::GateStatus;

pub mod statistical_engine;
pub use statistical_engine::{CanaryVerdict, MetricDistribution, StatisticalCanaryEngine};

/// Matches the `PreMergeCertificationReport` field name, so `unmeasured_gates`
/// names a gate a reader can look up in the fidelity registry.
const GATE_ID: &str = "automated_canary_status";

/// What must exist before a canary verdict can be reached at all. Published
/// verbatim as the `NotMeasured` reason.
const MISSING_METRICS_SOURCE: &str =
    "no canary deployment and no Prometheus or OpenTelemetry metrics endpoint are \
     configured, so no baseline or canary latency samples were ever read";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomatedCanaryReport {
    pub status: GateStatus,
    /// Whether a comparison was actually made AND found no regression. False
    /// while unmeasured: an unqueried canary cannot be asserted healthy.
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

    /// Judges a caller-supplied distribution. An empty distribution is not a
    /// healthy one: it is the absence of a measurement, and is reported as
    /// such.
    pub fn evaluate_canary(&self, distribution: &MetricDistribution) -> AutomatedCanaryReport {
        let verdict = self.engine.evaluate_canary_distributions(distribution);
        let (status, passed) = match &verdict {
            CanaryVerdict::NotMeasured => (Self::not_measured(), false),
            CanaryVerdict::Pass | CanaryVerdict::Marginal => (GateStatus::Passed, true),
            CanaryVerdict::Fail { reason, .. } => (GateStatus::Failed(reason.clone()), false),
        };

        AutomatedCanaryReport {
            status,
            passed,
            verdict,
        }
    }

    /// The review pipeline's entry point. Anvil drives no canary deployment and
    /// queries no metrics endpoint, so there is nothing to compare; see the
    /// module docs.
    pub fn evaluate_without_metrics_source(&self) -> AutomatedCanaryReport {
        AutomatedCanaryReport {
            status: Self::not_measured(),
            passed: false,
            verdict: CanaryVerdict::NotMeasured,
        }
    }

    fn not_measured() -> GateStatus {
        GateStatus::NotMeasured {
            gate_id: GATE_ID.to_string(),
            reason: MISSING_METRICS_SOURCE.to_string(),
        }
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
        assert!(matches!(report.status, GateStatus::Passed));
    }

    #[test]
    fn an_absent_metrics_source_is_not_a_healthy_canary() {
        let report = AutomatedCanaryAnalysis::new().evaluate_without_metrics_source();
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!report.passed, "absent evidence must not certify");
    }
}
