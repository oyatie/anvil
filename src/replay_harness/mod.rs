pub mod trace_replayer;

pub use trace_replayer::{ReplayTraceRecord, TraceReplayer};

#[derive(Clone, Debug)]
pub struct ReplayHarnessReport {
    pub passed: bool,
    pub replayed_fixtures_count: usize,
    pub divergence_detected: bool,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct DeterministicReplayHarness {
    replayer: TraceReplayer,
}

impl Default for DeterministicReplayHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterministicReplayHarness {
    pub fn new() -> Self {
        Self {
            replayer: TraceReplayer::new(),
        }
    }

    pub fn evaluate_replay_parity(&self, traces: &[ReplayTraceRecord]) -> ReplayHarnessReport {
        let (passed, count) = self.replayer.execute_replay_verification(traces);

        let summary = if passed {
            format!(
                "Deterministic Trace Replay PASSED: 100% byte-for-byte state parity across {} production fixtures.",
                count
            )
        } else {
            "Deterministic Trace Replay FAILED: State divergence detected during trace replay."
                .to_string()
        };

        ReplayHarnessReport {
            passed,
            replayed_fixtures_count: count,
            divergence_detected: !passed,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_harness_nominal() {
        let harness = DeterministicReplayHarness::new();
        let report = harness.evaluate_replay_parity(&[]);
        assert!(report.passed);
    }
}
