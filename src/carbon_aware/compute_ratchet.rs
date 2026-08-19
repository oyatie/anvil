#[derive(Clone, Debug, Default)]
pub struct ComputeRatchet;

impl ComputeRatchet {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate_carbon_intensity(
        &self,
        cpu_seconds_budget: f64,
        actual_cpu_seconds: f64,
    ) -> (bool, f64, bool) {
        // Average CPU package thermal design power: ~65 Watts -> Joules = Watts * Seconds
        let joules = actual_cpu_seconds * 65.0;
        let passed = actual_cpu_seconds <= cpu_seconds_budget;
        let schedule_green = !passed;

        (passed, joules, schedule_green)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evaluates_energy_budget() {
        let ratchet = ComputeRatchet::new();
        let (passed, joules, green) = ratchet.evaluate_carbon_intensity(20.0, 10.0);
        assert!(passed);
        assert_eq!(joules, 650.0);
        assert!(!green);
    }
}
