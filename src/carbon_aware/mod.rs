pub mod compute_ratchet;

use compute_ratchet::ComputeRatchet;

#[derive(Clone, Debug)]
pub struct CarbonComputeReport {
    pub passed: bool,
    pub estimated_joules_per_build: f64,
    pub green_window_scheduled: bool,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct CarbonAwareComputeRatchet {
    ratchet: ComputeRatchet,
}

impl Default for CarbonAwareComputeRatchet {
    fn default() -> Self {
        Self::new()
    }
}

impl CarbonAwareComputeRatchet {
    pub fn new() -> Self {
        Self {
            ratchet: ComputeRatchet::new(),
        }
    }

    pub fn evaluate_compute_carbon(
        &self,
        cpu_seconds_budget: f64,
        actual_cpu_seconds: f64,
    ) -> CarbonComputeReport {
        let (passed, joules, green_window) = self
            .ratchet
            .evaluate_carbon_intensity(cpu_seconds_budget, actual_cpu_seconds);

        let summary = if passed {
            format!(
                "GreenOps Compute Ratchet PASSED: {:.1} Joules/build within sustainability target.",
                joules
            )
        } else {
            format!(
                "GreenOps Compute Ratchet REGRESSION: {:.1} Joules exceeded energy budget. Routed heavy soak to green window.",
                joules
            )
        };

        CarbonComputeReport {
            passed,
            estimated_joules_per_build: joules,
            green_window_scheduled: green_window,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_carbon_aware_nominal() {
        let ratchet = CarbonAwareComputeRatchet::new();
        let report = ratchet.evaluate_compute_carbon(30.0, 15.0);
        assert!(report.passed);
    }
}
