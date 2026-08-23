pub mod quarantine_manager;

use quarantine_manager::QuarantineManager;

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "flake_quarantine_status";

const NO_RUN_HISTORY: &str = "no test-run history is retained, so no test can be shown to be \
     non-deterministic and there is no quarantine lane to isolate one into";

#[derive(Clone, Debug)]
pub struct FlakeQuarantineReport {
    pub status: GateStatus,
    pub passed: bool,
    pub quarantined_tests_isolated: usize,
    pub rehabilitated_tests_restored: usize,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct FlakeQuarantineLifecycle {
    manager: QuarantineManager,
}

impl Default for FlakeQuarantineLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl FlakeQuarantineLifecycle {
    pub fn new() -> Self {
        Self {
            manager: QuarantineManager::new(),
        }
    }

    pub fn evaluate_quarantine_lifecycle(
        &self,
        modified_tests: &[String],
    ) -> FlakeQuarantineReport {
        // `process_test_lifecycle` substring-matches "flaky" against the paths
        // it is handed. That says something about a filename and nothing about
        // whether a test is non-deterministic, which needs run history Anvil
        // does not keep. The counters are retained as data; the verdict is not
        // derived from them, because they are not evidence of flakiness.
        let (quarantined, rehabilitated) = self.manager.process_test_lifecycle(modified_tests);
        let passed = false;

        let summary = format!(
            "{NO_RUN_HISTORY} ({} path(s) matched the name heuristic, {} the rehabilitation one; neither is a flakiness measurement)",
            quarantined, rehabilitated
        );

        FlakeQuarantineReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: NO_RUN_HISTORY.to_string(),
            },
            passed,
            quarantined_tests_isolated: quarantined,
            rehabilitated_tests_restored: rehabilitated,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// This asserted `report.passed`, which was the literal `true` -- so it
    /// held for every input and tested nothing. It is the fourth test in this
    /// codebase found certifying its own gate's constant.
    ///
    /// No run history exists, so no input can make this gate a pass.
    #[test]
    fn no_input_makes_the_quarantine_gate_a_pass() {
        let lifecycle = FlakeQuarantineLifecycle::new();

        for paths in [
            vec!["test_normal_case".to_string()],
            vec!["tests/flaky_network_test.rs".to_string()],
            vec![],
        ] {
            let report = lifecycle.evaluate_quarantine_lifecycle(&paths);
            assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
            assert!(!report.passed, "no run history, so no verdict: {paths:?}");
        }
    }
}
