#[derive(Clone, Debug)]
pub struct RetryFinding {
    pub line_number: usize,
    pub has_jitter: bool,
    pub has_deadline_propagation: bool,
    pub snippet: String,
}

#[derive(Clone, Debug, Default)]
pub struct BackoffScanner;

impl BackoffScanner {
    pub fn new() -> Self {
        Self
    }

    pub fn scan_diff_for_unjittered_retries(&self, diff_content: &str) -> Vec<RetryFinding> {
        let mut findings = Vec::new();

        for (idx, line) in diff_content.lines().enumerate() {
            if !line.starts_with('+') {
                continue;
            }

            let lower = line.to_lowercase();
            if lower.contains("retry")
                || lower.contains("loop {")
                || lower.contains("tokio::time::sleep")
            {
                // Check if it includes jitter indicators
                let has_jitter = lower.contains("jitter")
                    || lower.contains("rand")
                    || lower.contains("random")
                    || lower.contains("exponential");

                // Check for deadline / timeout propagation
                let has_deadline = lower.contains("timeout")
                    || lower.contains("deadline")
                    || lower.contains("context")
                    || lower.contains("cancellation_token");

                if !has_jitter && (lower.contains("sleep") || lower.contains("retry")) {
                    findings.push(RetryFinding {
                        line_number: idx + 1,
                        has_jitter: false,
                        has_deadline_propagation: has_deadline,
                        snippet: line.trim().to_string(),
                    });
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catches_unjittered_fixed_sleep_retry() {
        let scanner = BackoffScanner::new();
        let diff = "+ loop { if let Err(_) = client.get().await { tokio::time::sleep(Duration::from_secs(1)).await; } }";
        let findings = scanner.scan_diff_for_unjittered_retries(diff);
        assert!(!findings.is_empty());
        assert!(!findings[0].has_jitter);
    }

    #[test]
    fn test_passes_jittered_exponential_backoff() {
        let scanner = BackoffScanner::new();
        let diff = "+ let duration = backoff.jittered_duration(attempt); tokio::time::sleep(duration).await;";
        let findings = scanner.scan_diff_for_unjittered_retries(diff);
        assert!(findings.is_empty() || findings[0].has_jitter);
    }
}
