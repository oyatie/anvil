//! The shape gate's outcome -> GateStatus mapping, as decided on 2026-08-20:
//! no spec is a visible Warning, an unreadable spec is NotMeasured, a blocking
//! regression fails, advisory-only and bootstrap warn with the distance.

use anvil::pre_merge_guard::GateStatus;
use anvil::pre_merge_guard::evaluator::shape_gate_status;
use anvil::shape::facade::gate::{ShapeGateOutcome, ShapeMeasurement};
use anvil::shape::ports::ShapeDistance;
use std::collections::BTreeMap;

fn measurement(advisory: usize) -> ShapeMeasurement {
    ShapeMeasurement {
        repo: "oyatie/anvil".into(),
        rev: "a".repeat(40),
        spec_source: "adopted".into(),
        distance: ShapeDistance {
            findings_total: 458,
            units_total: 89,
            units_conformant: 3,
            files_misplaced: 0,
            edges_denied: 200,
        },
        per_rule: BTreeMap::new(),
        blocking_regressions: 0,
        advisory_regressions: advisory,
        fixed: 0,
    }
}

#[test]
fn no_spec_is_a_visible_warning_that_does_not_withhold() {
    let s = shape_gate_status(&ShapeGateOutcome::NoSpec {
        reason: "no .anvil/shape.json at abc".into(),
    });
    match s {
        GateStatus::Warning(ref msg) => assert!(msg.contains("no shape spec adopted"), "{msg}"),
        other => panic!("{other:?}"),
    }
    assert!(s.is_acceptable());
    assert!(
        s.unmeasured_gate_id().is_none(),
        "a Warning does not withhold admission"
    );
}

#[test]
fn an_unreadable_spec_is_not_measured() {
    let s = shape_gate_status(&ShapeGateOutcome::SpecUnreadable {
        reason: "shape spec is invalid (1 problem(s))".into(),
    });
    assert_eq!(s.unmeasured_gate_id(), Some("shape_status"));
}

#[test]
fn a_git_failure_is_errored_and_withholds() {
    let s = shape_gate_status(&ShapeGateOutcome::Errored {
        reason: "git ls-tree failed".into(),
    });
    assert!(matches!(s, GateStatus::Errored(_)));
    assert!(!s.is_acceptable());
}

#[test]
fn blocking_regressions_fail_and_name_the_first_keys() {
    let s = shape_gate_status(&ShapeGateOutcome::Judged {
        measurement: measurement(0),
        blocking: vec!["face_edge_denied: src/x/core/a.rs->src/x/ports".into()],
    });
    match s {
        GateStatus::Failed(msg) => assert!(
            msg.contains("1 regression(s)") && msg.contains("face_edge_denied"),
            "{msg}"
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn advisory_only_and_bootstrap_warn_with_the_distance_and_clean_passes() {
    let s = shape_gate_status(&ShapeGateOutcome::Judged {
        measurement: measurement(200),
        blocking: vec![],
    });
    assert!(
        matches!(&s, GateStatus::Warning(m) if m.contains("distance 458") && m.contains("200 new on advisory"))
    );
    let s = shape_gate_status(&ShapeGateOutcome::Bootstrap {
        measurement: measurement(0),
    });
    assert!(matches!(&s, GateStatus::Warning(m) if m.contains("bootstraps")));
    let s = shape_gate_status(&ShapeGateOutcome::Judged {
        measurement: measurement(0),
        blocking: vec![],
    });
    assert_eq!(s, GateStatus::Passed);
}
