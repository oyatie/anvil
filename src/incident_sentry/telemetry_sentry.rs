use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveGoldenSignals {
    pub p99_latency_ms: f64,
    pub error_rate_pct: f64,
    pub panic_count_last_5m: u64,
    pub deployed_commit_sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentSentryDecision {
    pub is_healthy: bool,
    pub should_emergency_revert: bool,
    pub notice: String,
}

pub struct TelemetrySentry;

impl Default for TelemetrySentry {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetrySentry {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic evaluation of live production golden signals to trigger autonomous emergency reverts
    pub fn evaluate_production_health(
        &self,
        signals: &LiveGoldenSignals,
    ) -> IncidentSentryDecision {
        let is_unhealthy = signals.error_rate_pct > 0.5
            || signals.panic_count_last_5m > 0
            || signals.p99_latency_ms > 500.0;

        if is_unhealthy {
            IncidentSentryDecision {
                is_healthy: false,
                should_emergency_revert: true,
                notice: format!(
                    "🚨 CRITICAL PRODUCTION SLO BREACH: error_rate={:.2}%, p99={}ms, panics={}. Auto-triggering emergency git revert of commit {}.",
                    signals.error_rate_pct, signals.p99_latency_ms, signals.panic_count_last_5m, signals.deployed_commit_sha
                ),
            }
        } else {
            IncidentSentryDecision {
                is_healthy: true,
                should_emergency_revert: false,
                notice: format!(
                    "✅ Production golden signals healthy: error_rate={:.2}%, p99={:.1}ms, 0 panics.",
                    signals.error_rate_pct, signals.p99_latency_ms
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trips_on_production_panics() {
        let sentry = TelemetrySentry::new();
        let signals = LiveGoldenSignals {
            p99_latency_ms: 120.0,
            error_rate_pct: 0.01,
            panic_count_last_5m: 3,
            deployed_commit_sha: "bad_sha_123".to_string(),
        };

        let decision = sentry.evaluate_production_health(&signals);
        assert!(!decision.is_healthy);
        assert!(decision.should_emergency_revert);
        assert!(decision.notice.contains("bad_sha_123"));
    }
}
