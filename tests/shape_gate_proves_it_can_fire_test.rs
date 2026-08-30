//! The shape gate demonstrates both halves, and its third answer.
//!
//! `shape_gate_status` is the conversion the review pipeline publishes. Its
//! interesting property is that it distinguishes four outcomes a bool cannot:
//! a regression on a blocking rule (`Failed`), an advisory drift (`Warning`),
//! a spec that could not be read (`NotMeasured`), and a clean judgement
//! (`Passed`). A gate that collapsed the third into a pass would publish
//! "conformant" for a spec it never read, which is I1.

use anvil::pre_merge_guard::evaluator::shape_gate_status;
use anvil::pre_merge_guard::report::GateStatus;
use anvil::shape::core::report::ShapeDistance;
use anvil::shape::facade::gate::{ShapeGateOutcome, ShapeMeasurement};

fn measurement(blocking: usize, advisory: usize) -> ShapeMeasurement {
    ShapeMeasurement {
        repo: "oyatie/anvil".to_string(),
        rev: "cafe1234".to_string(),
        spec_source: ".anvil/shape.json".to_string(),
        distance: ShapeDistance {
            findings_total: blocking + advisory,
            units_total: 10,
            units_conformant: 10 - blocking,
            files_misplaced: 0,
            edges_denied: blocking,
        },
        per_rule: Default::default(),
        blocking_regressions: blocking,
        advisory_regressions: advisory,
        fixed: 0,
    }
}

#[test]
fn shape_fires_on_a_regression_against_a_blocking_rule() {
    let status = shape_gate_status(&ShapeGateOutcome::Judged {
        blocking: vec!["edges_denied: cli -> webhook".to_string()],
        measurement: measurement(1, 0),
    });
    assert!(
        matches!(status, GateStatus::Failed(_)),
        "a blocking shape rule regressed against the baseline and the gate did \
         not withhold the merge: {status:?}"
    );
}

#[test]
fn shape_spares_a_judgement_with_no_regression() {
    let status = shape_gate_status(&ShapeGateOutcome::Judged {
        blocking: Vec::new(),
        measurement: measurement(0, 0),
    });
    assert!(
        matches!(status, GateStatus::Passed),
        "nothing regressed, so the gate must not withhold: {status:?}"
    );
}

/// The answer a bool cannot carry: the spec was never read.
#[test]
fn shape_withholds_when_the_spec_could_not_be_read() {
    let status = shape_gate_status(&ShapeGateOutcome::SpecUnreadable {
        reason: "`.anvil/shape.json` is not valid JSON".to_string(),
    });
    assert!(
        matches!(status, GateStatus::NotMeasured { .. }),
        "an unreadable spec is not a conformant tree. Publishing `Passed` here \
         would report conformance to a specification nobody read: {status:?}"
    );
}

/// And an advisory drift is a warning, not a refusal.
#[test]
fn shape_warns_rather_than_refuses_on_advisory_drift() {
    let status = shape_gate_status(&ShapeGateOutcome::Judged {
        blocking: Vec::new(),
        measurement: measurement(0, 3),
    });
    assert!(
        matches!(status, GateStatus::Warning(_)),
        "advisory rules are advisory; refusing on them makes every advisory \
         rule a blocking one: {status:?}"
    );
}
