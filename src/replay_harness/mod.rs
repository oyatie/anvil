pub mod trace_replayer;

pub use trace_replayer::{ReplayTraceRecord, TraceReplayer};

use crate::pre_merge_guard::GateStatus;

const GATE_ID: &str = "replay_harness_status";

const MISSING_TRACE_SOURCE: &str = "no production trace corpus was read, so nothing was replayed and \
     byte-for-byte parity is unknown for this pull request";

#[derive(Clone, Debug)]
pub struct ReplayHarnessReport {
    pub status: GateStatus,
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

    /// The gate's answer when no trace corpus was supplied.
    ///
    /// The pipeline passed `&[]` on every PR and the replayer answered it with
    /// `(true, 5)`, publishing parity across five fixtures that never existed.
    /// Absent input is not a clean replay.
    pub fn evaluate_without_trace_source(&self) -> ReplayHarnessReport {
        ReplayHarnessReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_TRACE_SOURCE.to_string(),
            },
            passed: false,
            replayed_fixtures_count: 0,
            divergence_detected: false,
            summary: MISSING_TRACE_SOURCE.to_string(),
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
            status: if passed {
                GateStatus::Passed
            } else {
                GateStatus::Failed(summary.clone())
            },
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

#[cfg(test)]
mod no_trace_source_tests {
    use super::*;

    /// The review pipeline called `evaluate_replay_parity(&[])` on every PR,
    /// and `execute_replay_verification` answered an empty slice with
    /// `(true, 5)` -- a hardcoded count -- so the scorecard published
    /// "100% byte-for-byte state parity across 5 production fixtures" about
    /// five fixtures that do not exist and were never replayed.
    #[test]
    fn absent_traces_are_reported_as_unmeasured_not_as_five_replays() {
        let report = DeterministicReplayHarness::new().evaluate_without_trace_source();

        assert_eq!(
            report.status.unmeasured_gate_id(),
            Some(GATE_ID),
            "with no trace corpus the gate must report NotMeasured"
        );
        assert!(!report.passed, "an unreplayed corpus is not a pass");
        assert_eq!(
            report.replayed_fixtures_count, 0,
            "nothing was replayed, so no count may be published"
        );
        assert!(
            !report.summary.contains('5'),
            "the summary must not quote a fabricated fixture count: {}",
            report.summary
        );
    }

    /// The measuring path still has to measure -- reporting NotMeasured for
    /// everything would satisfy the test above while gating nothing.
    #[test]
    fn a_real_trace_corpus_is_still_replayed_and_judged() {
        let harness = DeterministicReplayHarness::new();

        let good = harness.evaluate_replay_parity(&[ReplayTraceRecord {
            trace_id: "t1".to_string(),
            input_payload: "{\"k\":1}".to_string(),
            expected_output: "{\"ok\":true}".to_string(),
        }]);
        assert!(good.passed, "a replayable trace must pass");
        assert_eq!(good.replayed_fixtures_count, 1);

        let bad = harness.evaluate_replay_parity(&[ReplayTraceRecord {
            trace_id: "t2".to_string(),
            input_payload: String::new(),
            expected_output: "{\"ok\":true}".to_string(),
        }]);
        assert!(!bad.passed, "an empty payload must not replay clean");
    }

    /// Pins the deleted fabrication directly: even reached with an empty
    /// slice, the replayer must not invent a fixture count.
    #[test]
    fn an_empty_trace_slice_never_reports_replayed_fixtures() {
        let (_, count) = TraceReplayer::new().execute_replay_verification(&[]);
        assert_eq!(count, 0, "no traces were replayed, so the count must be 0");
    }
}
