//! PreMergeGuard: the live gate corpus, certification, and governance matrix.
//! The count is `TOTAL_GATES`, never a number written in prose.

pub mod admission;
pub mod evaluator;
pub mod gate_labels;
pub mod matrix;
pub mod report;
pub mod scanner;
pub mod status;

pub use admission::{Absence, absence_blocks, absence_of};
pub use evaluator::PreMergeGuard;
pub use matrix::MatrixRenderer;
pub use report::{GateStatus, PreMergeCertificationReport};
pub use scanner::PreMergeScanner;

/// Gates this build cannot measure **and whose absence withholds the merge**,
/// or `None` when nothing it cannot measure would block anything.
///
/// The filter is the whole point, and its absence is what broke every door.
/// This was written when `admission_refusal` refused a report in which ANY
/// gate produced no measurement, so anything unmeasurable made every report
/// inadmissible. `ABSENCE_POLICY` replaced that premise: a `NotMeasured` gate
/// withholds admission only when `absence_blocks` says so, and `slo_status` is
/// declared `NotProvisioned` -- this deployment has no telemetry endpoint, the
/// capability is absent, and its absence is not a defect.
///
/// The stale premise survived because nothing re-read it. The three enlist
/// doors kept refusing every input for a reason that had stopped being true,
/// which is how the certification corpus stopped being consulted at all.
///
/// Kept as a cheap pre-flight: it is a fact about the deployment, knowable
/// before a single guard runs, and it says nothing about any pull request. The
/// review pipeline still runs the corpus and still publishes the real
/// `NotMeasured` reason on the scorecard.
pub fn unmeasurable_gates_in_this_build() -> Option<String> {
    let mut candidates: Vec<(&str, String)> = Vec::new();
    if let Some(reason) = crate::slo_canary_guard::burn_rate_is_unmeasurable() {
        candidates.push(("slo_status", reason.to_string()));
    }
    blocking_unmeasurable(&candidates)
}

/// The pure half: keep only those whose absence actually blocks admission.
///
/// Separated so it can be proven in both directions. A build with no blocking
/// candidate must return `None`, and one with a blocking candidate must name
/// it -- neither is demonstrable while the only input is whatever this
/// deployment happens to lack.
pub fn blocking_unmeasurable(candidates: &[(&str, String)]) -> Option<String> {
    let blocked: Vec<String> = candidates
        .iter()
        .filter(|(gate, _)| absence_blocks(gate))
        .map(|(gate, reason)| format!("`{gate}` can never pass: {reason}"))
        .collect();
    (!blocked.is_empty()).then(|| blocked.join("; "))
}
