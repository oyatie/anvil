//! The Shape Program outcome, in the certification vocabulary.
//!
//! A module of its own, on both sides. The shape unit stays free of gate ids
//! and `GateStatus`; the evaluator holds the wiring, and it is far enough past
//! the file budget that a decision worth reading does not belong inside it.

use super::report::GateStatus;

const GATE_ID: &str = "shape_status";

/// Maps the Shape Program outcome onto the certification vocabulary.
///
/// - No spec adopted: `Warning`, visible on every scorecard, never
///   withholding — a tenant that has not opted in has nothing to measure
///   (owner decision 2026-08-20; precedent: coverage's NothingToMeasure).
/// - Spec present but unreadable: `NotMeasured` (I1 — the gate was asked to
///   measure and could not).
/// - Git failure: `Errored`.
/// - Bootstrap (no baseline at the merge-base) and advisory-only regressions:
///   `Warning` carrying the distance.
/// - Any regression on a blocking rule: `Failed`, first five keys named.
/// - A blocking rule the engine could not evaluate: `NotMeasured`. It found
///   nothing only because it never ran, and a judgement standing on a rule
///   that did not run is absent evidence reading as a pass (I1).
pub fn shape_gate_status(outcome: &crate::shape::facade::gate::ShapeGateOutcome) -> GateStatus {
    use crate::shape::facade::gate::ShapeGateOutcome as O;
    match outcome {
        O::NoSpec { .. } => GateStatus::Warning(
            "no shape spec adopted (.anvil/shape.json absent); see `anvil shape validate-spec`"
                .to_string(),
        ),
        O::SpecUnreadable { reason } => GateStatus::NotMeasured {
            gate_id: GATE_ID.to_string(),
            reason: reason.clone(),
        },
        O::Errored { reason } => GateStatus::Errored(reason.clone()),
        O::Bootstrap { .. } => GateStatus::Warning(outcome.summary()),
        O::Judged {
            blocking,
            measurement,
        } => {
            if !blocking.is_empty() {
                let mut first: Vec<&str> = blocking.iter().take(5).map(String::as_str).collect();
                if blocking.len() > 5 {
                    first.push("…");
                }
                GateStatus::Failed(format!(
                    "{} regression(s) on blocking shape rules since the baseline: {}",
                    blocking.len(),
                    first.join("; ")
                ))
            } else if let Some(reason) = measurement.unmeasured_reason() {
                GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason,
                }
            } else if measurement.advisory_regressions > 0 {
                GateStatus::Warning(outcome.summary())
            } else {
                GateStatus::Passed
            }
        }
    }
}
