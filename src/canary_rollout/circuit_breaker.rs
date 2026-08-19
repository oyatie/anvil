use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanaryMetricsSnapshot {
    pub step_traffic_percent: usize,
    pub p99_latency_ms: f64,
    pub error_rate_percent: f64,
    pub burn_rate_5m: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerDecision {
    pub should_rollback: bool,
    pub reason: Option<String>,
}

pub struct CanaryCircuitBreaker;

impl Default for CanaryCircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl CanaryCircuitBreaker {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic evaluation of progressive rollout health against SRE SLO error budget thresholds
    pub fn evaluate_metrics(
        &self,
        metrics: &CanaryMetricsSnapshot,
        max_burn_rate_ceiling: f64,
        max_p99_latency_ms: f64,
    ) -> CircuitBreakerDecision {
        if metrics.burn_rate_5m > max_burn_rate_ceiling {
            return CircuitBreakerDecision {
                should_rollback: true,
                reason: Some(format!(
                    "Canary 5-minute error budget burn rate ({:.2}x) exceeded release threshold ({:.1}x). Auto-tripping circuit breaker.",
                    metrics.burn_rate_5m, max_burn_rate_ceiling
                )),
            };
        }

        if metrics.p99_latency_ms > max_p99_latency_ms {
            return CircuitBreakerDecision {
                should_rollback: true,
                reason: Some(format!(
                    "Canary p99 latency ({:.1}ms) exceeded maximum latency ceiling ({:.1}ms). Auto-tripping circuit breaker.",
                    metrics.p99_latency_ms, max_p99_latency_ms
                )),
            };
        }

        CircuitBreakerDecision {
            should_rollback: false,
            reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_trips_on_high_burn_rate() {
        let cb = CanaryCircuitBreaker::new();
        let metrics = CanaryMetricsSnapshot {
            step_traffic_percent: 10,
            p99_latency_ms: 45.0,
            error_rate_percent: 2.5,
            burn_rate_5m: 4.8, // > 3.0x
        };
        let decision = cb.evaluate_metrics(&metrics, 3.0, 200.0);
        assert!(decision.should_rollback);
    }

    #[test]
    fn test_circuit_breaker_passes_nominal_traffic() {
        let cb = CanaryCircuitBreaker::new();
        let metrics = CanaryMetricsSnapshot {
            step_traffic_percent: 25,
            p99_latency_ms: 32.0,
            error_rate_percent: 0.01,
            burn_rate_5m: 0.4,
        };
        let decision = cb.evaluate_metrics(&metrics, 3.0, 200.0);
        assert!(!decision.should_rollback);
    }
}
