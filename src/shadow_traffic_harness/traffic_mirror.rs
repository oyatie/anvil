use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowTrafficMetrics {
    pub sampled_requests: usize,
    pub payload_parity_pct: f64,
    pub status_code_parity_pct: f64,
    pub latency_delta_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowComparisonResult {
    pub is_parity_satisfied: bool,
    pub details: String,
}

pub struct TrafficMirrorComparator;

impl Default for TrafficMirrorComparator {
    fn default() -> Self {
        Self::new()
    }
}

impl TrafficMirrorComparator {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic evaluation of dark-traffic shadow replay parity
    pub fn evaluate_shadow_parity(&self, metrics: &ShadowTrafficMetrics) -> ShadowComparisonResult {
        let is_parity_satisfied =
            metrics.payload_parity_pct >= 99.5 && metrics.status_code_parity_pct >= 99.9;

        let details = if is_parity_satisfied {
            format!(
                "✅ Dark-traffic shadow replay verified: {:.2}% payload parity, {:.2}% status code parity across {} sampled requests.",
                metrics.payload_parity_pct,
                metrics.status_code_parity_pct,
                metrics.sampled_requests
            )
        } else {
            format!(
                "❌ Shadow parity mismatch: payload parity ({:.2}%) or status code parity ({:.2}%) fell below 99.5% threshold.",
                metrics.payload_parity_pct, metrics.status_code_parity_pct
            )
        };

        ShadowComparisonResult {
            is_parity_satisfied,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_parity_passed() {
        let comp = TrafficMirrorComparator::new();
        let metrics = ShadowTrafficMetrics {
            sampled_requests: 10000,
            payload_parity_pct: 99.95,
            status_code_parity_pct: 100.0,
            latency_delta_pct: 1.2,
        };
        let res = comp.evaluate_shadow_parity(&metrics);
        assert!(res.is_parity_satisfied);
    }
}
