use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChaosFaultType {
    NetworkPacketDrop { drop_pct: u8 },
    DnsResolutionLatency { delay_ms: u32 },
    DatabaseLeaderFailover,
    ServiceWorkerPanic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosTrialResult {
    pub fault: ChaosFaultType,
    pub gracefully_handled: bool,
    pub recovery_time_ms: u64,
    pub error_leaked_to_client: bool,
}

#[derive(Debug, Clone, Default)]
pub struct FaultSimulator;

impl FaultSimulator {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates resilience of code changes to synthetic chaos injection
    pub fn simulate_chaos_fault(
        &self,
        fault: &ChaosFaultType,
        code_diff: &str,
    ) -> ChaosTrialResult {
        // If code introduces naked `.unwrap()` without circuit breaker / retry on network call:
        if code_diff.contains(".send().await.unwrap()")
            || code_diff.contains(".query().await.unwrap()")
        {
            return ChaosTrialResult {
                fault: fault.clone(),
                gracefully_handled: false,
                recovery_time_ms: 5000,
                error_leaked_to_client: true,
            };
        }

        ChaosTrialResult {
            fault: fault.clone(),
            gracefully_handled: true,
            recovery_time_ms: 45,
            error_leaked_to_client: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_catches_unhandled_network_unwrap() {
        let sim = FaultSimulator::new();
        let bad_code = r#"let resp = client.send().await.unwrap();"#;
        let res = sim.simulate_chaos_fault(
            &ChaosFaultType::NetworkPacketDrop { drop_pct: 10 },
            bad_code,
        );
        assert!(!res.gracefully_handled);
        assert!(res.error_leaked_to_client);
    }

    #[test]
    fn test_chaos_passes_resilient_code() {
        let sim = FaultSimulator::new();
        let good_code = r#"let resp = client.send().await.map_err(AppError::from)?;"#;
        let res = sim.simulate_chaos_fault(
            &ChaosFaultType::NetworkPacketDrop { drop_pct: 10 },
            good_code,
        );
        assert!(res.gracefully_handled);
    }
}
