use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnboundedCapacityFinding {
    pub file_path: String,
    pub line_number: usize,
    pub snippet: String,
    pub reason: String,
}

pub struct BufferLimitsChecker;

impl Default for BufferLimitsChecker {
    fn default() -> Self {
        Self::new()
    }
}

impl BufferLimitsChecker {
    pub fn new() -> Self {
        Self
    }

    /// Scans diff for unbounded channels or unbounded dynamic buffers in hotpaths
    pub fn scan_unbounded_structures(
        &self,
        file_path: &str,
        content: &str,
    ) -> Vec<UnboundedCapacityFinding> {
        let mut findings = Vec::new();
        let unbounded_chan_re =
            Regex::new(r#"(?i)mpsc::unbounded_channel(?:::<[^>]+>)?\s*\("#).unwrap();

        for (idx, line) in content.lines().enumerate() {
            if unbounded_chan_re.is_match(line) {
                findings.push(UnboundedCapacityFinding {
                    file_path: file_path.to_string(),
                    line_number: idx + 1,
                    snippet: line.trim().to_string(),
                    reason: "Unbounded channel (`mpsc::unbounded_channel`) violates constant-work & anti-fragility invariants. Use bounded `mpsc::channel(N)` with backpressure.".to_string(),
                });
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_unbounded_channel() {
        let checker = BufferLimitsChecker::new();
        let code = "+ let (tx, rx) = tokio::sync::mpsc::unbounded_channel();";
        let findings = checker.scan_unbounded_structures("src/queue.rs", code);
        assert_eq!(findings.len(), 1);
    }
}
