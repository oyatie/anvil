//! PreMergeGuard: the live gate corpus, certification, and governance matrix.
//! The count is `TOTAL_GATES`, never a number written in prose.

pub mod admission;
pub mod evaluator;
pub mod matrix;
pub mod report;
pub mod scanner;

pub use admission::{Absence, absence_blocks, absence_of};
pub use evaluator::PreMergeGuard;
pub use matrix::MatrixRenderer;
pub use report::{GateStatus, PreMergeCertificationReport};
pub use scanner::PreMergeScanner;

/// Gates this build cannot produce a measurement for, whatever the pull request
/// is, or `None` when every gate can at least be measured.
///
/// `PreMergeCertificationReport::admission_refusal` refuses a report in which
/// any gate produced no measurement, so a gate that is unmeasurable by
/// construction makes every report this build can produce inadmissible. That is
/// a fact about the deployment, knowable before a single guard runs, and the
/// enlist doors read it so they do not pay for a corpus to discover it. It says
/// nothing about any pull request and is not a substitute for running the gate:
/// the review pipeline still runs the corpus and still publishes the real
/// `NotMeasured` status with the real reason on the scorecard.
///
/// One entry today. `SloCanaryGuard::evaluate` has three outcomes -- `Failed`
/// for a defect in an OpenSLO spec, `Errored` for a spec it could not read, and
/// `NotMeasured` otherwise -- and no `Passed` branch at all, because nothing in
/// this crate queries a telemetry endpoint. All three are refused, so
/// `slo_status` cannot admit anything in any execution of this build. Adding a
/// telemetry integration is what removes this, and the day it does the
/// condition below stops holding on its own rather than needing to be
/// remembered.
pub fn unmeasurable_gates_in_this_build() -> Option<String> {
    let mut blocked: Vec<String> = Vec::new();
    if let Some(reason) = crate::slo_canary_guard::burn_rate_is_unmeasurable() {
        blocked.push(format!("`slo_status` can never pass: {reason}"));
    }
    (!blocked.is_empty()).then(|| blocked.join("; "))
}
