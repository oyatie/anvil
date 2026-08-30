//! Publishing a harness verdict as a gate status.
//!
//! The join between the two vocabularies, and the whole of I1 lives in it. The
//! certification report speaks [`GateStatus`]; the harness speaks
//! [`Evaluated`], which has exactly two variants and no way to spell "examined
//! nothing, found nothing".
//!
//! The mapping must not reintroduce what the types removed. `Withheld` becomes
//! `NotMeasured` under the gate's own id, which blocks merge-queue admission
//! through `unmeasured_gates` while making no accusation against the pull
//! request. It never becomes `Passed`, and the test beside this seeds that.

use super::{Evaluated, Withheld};
use crate::pre_merge_guard::report::GateStatus;

/// Why a rule did not run, in a sentence a reviewer can act on.
fn reason(w: &Withheld) -> String {
    match w {
        Withheld::Undeclared => {
            "the rule is registered but this run did not declare it".to_string()
        }
        Withheld::InputsAbsent { needed } => {
            format!("the corpus does not hold {needed:?}, which this rule requires")
        }
        Withheld::FixtureFailed { detail } => {
            format!(
                "the rule's own fixture stopped behaving, so its verdict is not trusted: {detail}"
            )
        }
        Withheld::Unclassifiable { subjects } => format!(
            "{} subject(s) could not be classified, and an input nobody could \
             classify is not an input that passed",
            subjects.len()
        ),
    }
}

/// The gate status a harness verdict is entitled to publish.
pub fn publish(gate_id: &str, evaluated: &Evaluated) -> GateStatus {
    match evaluated {
        Evaluated::Withheld(w) => GateStatus::NotMeasured {
            gate_id: gate_id.to_string(),
            reason: reason(w),
        },
        Evaluated::Measured {
            subjects_seen,
            findings,
        } if findings.is_empty() => {
            let _ = subjects_seen;
            GateStatus::Passed
        }
        Evaluated::Measured {
            subjects_seen,
            findings,
        } => GateStatus::Failed(format!(
            "{} finding(s) over {} subject(s) examined: {}",
            findings.len(),
            subjects_seen,
            findings
                .iter()
                .map(|f| f.detail.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        )),
    }
}
