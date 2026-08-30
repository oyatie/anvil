//! The named gate conversions.
//!
//! Split from `evaluator` because a decision inside a sixty-nine-argument
//! function is a decision no test can reach, and `gate_proof` asks every gate
//! to demonstrate that it both fires and discriminates. Each of these was
//! inline until a proof needed to call it.

use super::report::GateStatus;

/// The gate an unresolved-thread report produces.
///
/// Named, and outside `evaluate_pre_merge_gates`, because it is the middle
/// link of the chain Issue #18 asks for -- GitHub's `isResolved`, this
/// conversion, then `admission_refusal` -- and a link inside a
/// twenty-five-argument function is a link no test can reach. The check that
/// an unresolved thread FAILS the gate used to be
/// `assert!(!report.is_clean)` against a struct literal that set
/// `is_clean: false`: it restated its own fixture and would have passed had
/// this conversion been inverted.
pub fn unresolved_review_gate(
    report: &crate::unresolved_review_guard::UnresolvedReviewReport,
) -> GateStatus {
    if report.is_clean {
        GateStatus::Passed
    } else {
        GateStatus::Failed(report.summary.clone())
    }
}

/// The gate a review verdict produces.
///
/// Named, like `unresolved_review_gate`: a decision inside a sixty-nine-argument
/// function is one no test can reach. `VERDICT_ERRORED` is `Errored`, never
/// `Failed` — a review that did not complete judged nothing, and reporting a
/// refusal it never made is I1's symmetric violation.
pub fn review_verdict_gate(verdict: &str) -> GateStatus {
    match verdict {
        "APPROVE" | "COMMENT" => GateStatus::Passed,
        crate::reviewer::VERDICT_ERRORED => GateStatus::Errored(
            "AI Code Review produced no parseable verdict; the review did not complete".to_string(),
        ),
        other => GateStatus::Failed(format!(
            "AI Code Review & 16-Lens Matrix issued blocking verdict: {}",
            other
        )),
    }
}
