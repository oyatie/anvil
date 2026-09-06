pub mod compute_ratchet;

use compute_ratchet::ComputeRatchet;

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "carbon_compute_status";

const MISSING_ENERGY_SOURCE: &str = "no CPU-time or grid-intensity reading was taken, so this build's \
     energy cost is unknown";

#[derive(Clone, Debug)]
pub struct CarbonComputeReport {
    pub status: GateStatus,
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

    /// The gate's answer when no energy reading was taken.
    ///
    /// The pipeline supplied the literals `30.0` and `12.0`, so the ratchet
    /// compared two constants and published joules derived from them.
    pub fn evaluate_without_energy_source(&self) -> CarbonComputeReport {
        CarbonComputeReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_ENERGY_SOURCE.to_string(),
            },
            passed: false,
            estimated_joules_per_build: 0.0,
            green_window_scheduled: false,
            summary: MISSING_ENERGY_SOURCE.to_string(),
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
            status: if passed {
                GateStatus::Passed
            } else {
                GateStatus::Warning(summary.clone())
            },
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

#[cfg(test)]
mod no_energy_source_tests {
    use super::*;

    /// The review pipeline called this with the literals `(30.0, 12.0)` on
    /// every PR -- a budget and an actual that were never measured -- so the
    /// gate always passed and published a joules figure derived from the two
    /// constants: "GreenOps Compute Ratchet PASSED: N Joules/build within
    /// sustainability target."
    #[test]
    fn absent_energy_readings_are_unmeasured_not_a_joules_figure() {
        let report = CarbonAwareComputeRatchet::new().evaluate_without_energy_source();

        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!report.passed);
        assert_eq!(
            report.estimated_joules_per_build, 0.0,
            "no CPU time was observed, so no energy figure may be published"
        );
    }

    /// The measuring path must still fail a build over its budget.
    #[test]
    fn exceeding_the_cpu_budget_still_fails() {
        let report = CarbonAwareComputeRatchet::new().evaluate_compute_carbon(10.0, 90.0);
        assert!(
            !report.passed,
            "90 CPU-seconds against a 10 budget must fail"
        );
    }
}
