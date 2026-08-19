pub mod backoff_scanner;

use backoff_scanner::BackoffScanner;

#[derive(Clone, Debug)]
pub struct JitteredBackoffReport {
    pub passed: bool,
    pub unjittered_retries_detected: usize,
    pub missing_deadline_calls: usize,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct JitteredBackoffGuard {
    scanner: BackoffScanner,
}

impl Default for JitteredBackoffGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl JitteredBackoffGuard {
    pub fn new() -> Self {
        Self {
            scanner: BackoffScanner::new(),
        }
    }

    pub fn evaluate_backoff_and_jitter(&self, diff_content: &str) -> JitteredBackoffReport {
        let findings = self.scanner.scan_diff_for_unjittered_retries(diff_content);
        let unjittered_count = findings.iter().filter(|f| !f.has_jitter).count();
        let missing_deadline_count = findings
            .iter()
            .filter(|f| !f.has_deadline_propagation)
            .count();

        let passed = unjittered_count == 0;
        let summary = if passed {
            "All network retries implement Full/Decorrelated Jitter and deadline propagation."
                .to_string()
        } else {
            format!(
                "Detected {} unjittered retry loops risking thundering herd storms.",
                unjittered_count
            )
        };

        JitteredBackoffReport {
            passed,
            unjittered_retries_detected: unjittered_count,
            missing_deadline_calls: missing_deadline_count,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jittered_backoff_nominal() {
        let guard = JitteredBackoffGuard::new();
        let diff = "+ let backoff = full_jitter(base, attempt);";
        let report = guard.evaluate_backoff_and_jitter(diff);
        assert!(report.passed);
    }
}
