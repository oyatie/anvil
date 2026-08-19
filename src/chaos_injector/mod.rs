use serde::{Deserialize, Serialize};

pub mod fault_simulator;
pub use fault_simulator::{ChaosFaultType, ChaosTrialResult, FaultSimulator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChaosInjectorReport {
    pub passed: bool,
    pub trials: Vec<ChaosTrialResult>,
}

#[derive(Debug, Clone, Default)]
pub struct ChaosFaultInjector {
    simulator: FaultSimulator,
}

impl ChaosFaultInjector {
    pub fn new() -> Self {
        Self {
            simulator: FaultSimulator::new(),
        }
    }

    pub fn inject_synthetic_chaos(
        &self,
        diff_content: &str,
    ) -> ChaosInjectorReport {
        let faults = vec![
            ChaosFaultType::NetworkPacketDrop { drop_pct: 5 },
            ChaosFaultType::DnsResolutionLatency { delay_ms: 250 },
            ChaosFaultType::DatabaseLeaderFailover,
        ];

        let mut trials = Vec::new();
        let mut all_passed = true;

        for fault in faults {
            let res = self.simulator.simulate_chaos_fault(&fault, diff_content);
            if !res.gracefully_handled {
                all_passed = false;
            }
            trials.push(res);
        }

        ChaosInjectorReport {
            passed: all_passed,
            trials,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chaos_injector_nominal() {
        let injector = ChaosFaultInjector::new();
        let report = injector.inject_synthetic_chaos("let x = 100;");
        assert!(report.passed);
        assert_eq!(report.trials.len(), 3);
    }
}
