#[derive(Clone, Debug, Default)]
pub struct QuarantineManager;

impl QuarantineManager {
    pub fn new() -> Self {
        Self
    }

    pub fn process_test_lifecycle(&self, modified_tests: &[String]) -> (usize, usize) {
        let mut quarantined = 0;
        let mut rehabilitated = 0;

        for test in modified_tests {
            if test.contains("flaky") || test.contains("non_deterministic") {
                quarantined += 1;
            } else if test.contains("rehabilitated") || test.contains("fixed_timing") {
                rehabilitated += 1;
            }
        }

        (quarantined, rehabilitated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isolates_flaky_test() {
        let manager = QuarantineManager::new();
        let (q, r) = manager.process_test_lifecycle(&["test_flaky_network_socket".to_string()]);
        assert_eq!(q, 1);
        assert_eq!(r, 0);
    }

    #[test]
    fn test_rehabilitates_fixed_test() {
        let manager = QuarantineManager::new();
        let (q, r) = manager.process_test_lifecycle(&["test_fixed_timing_lock".to_string()]);
        assert_eq!(q, 0);
        assert_eq!(r, 1);
    }
}
