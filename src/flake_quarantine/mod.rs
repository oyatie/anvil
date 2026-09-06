pub mod quarantine_manager;

use quarantine_manager::QuarantineManager;

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "flake_quarantine_status";

const NO_RUN_HISTORY: &str = "no test-run history is retained, so no test can be shown to be \
     non-deterministic and there is no quarantine lane to isolate one into";

const NO_QUARANTINE_LEDGER: &str = "no quarantine ledger is retained, so no test in this \
     repository is known to be quarantined and there is nothing to rehabilitate";

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

    /// The tests currently held in quarantine, or `None` when no ledger of
    /// quarantine membership is retained.
    pub fn retained_quarantine_set(&self) -> Option<Vec<String>> {
        self.manager.retained_quarantine_set()
    }

    /// What rehabilitation can say about the set that is actually quarantined.
    ///
    /// Three states an operator has to be able to tell apart: no ledger, an
    /// empty ledger, and members to evaluate. A caller that supplies a test
    /// name of its own evaluates the lifecycle of a fiction and prints an
    /// outcome for it, which reads exactly like an outcome for this repository.
    pub fn rehabilitation_report(&self) -> String {
        let Some(quarantined) = self.retained_quarantine_set() else {
            return NO_QUARANTINE_LEDGER.to_string();
        };
        if quarantined.is_empty() {
            return "the quarantine ledger is empty, so there is nothing to rehabilitate"
                .to_string();
        }
        let report = self.evaluate_quarantine_lifecycle(&quarantined);
        format!(
            "{}\nQuarantined: {} | Rehabilitated: {}",
            report.summary, report.quarantined_tests_isolated, report.rehabilitated_tests_restored
        )
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
