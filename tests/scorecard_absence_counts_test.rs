//! In-memory report composition; no forge, command or filesystem fixture.
use anvil::pre_merge_guard::{GateStatus, PreMergeCertificationReport};
use anvil::publish::scorecard;

fn report(overrides: &[(&str, GateStatus)]) -> PreMergeCertificationReport {
    let outcomes: Vec<_> = anvil::pre_merge_guard::matrix::GATE_LABELS
        .iter()
        .map(|(id, _, _)| {
            let status = overrides
                .iter()
                .find(|(key, _)| key == id)
                .map(|(_, status)| status.clone())
                .unwrap_or(GateStatus::Passed);
            (*id, status)
        })
        .collect();
    PreMergeCertificationReport::from_gate_outcomes(&outcomes).unwrap()
}

fn unmeasured(id: &str) -> GateStatus {
    GateStatus::NotMeasured {
        gate_id: id.into(),
        reason: "capability absent".into(),
    }
}

fn inapplicable(id: &str) -> GateStatus {
    GateStatus::NotApplicable {
        gate_id: id.into(),
        subject: "no applicable input in this change".into(),
    }
}

#[test]
fn mixed_nonblocking_absences_render_without_underflow_or_fabricated_passes() {
    let report = report(&[
        ("slo_status", unmeasured("slo_status")),
        ("trace_status", inapplicable("trace_status")),
    ]);
    assert!(report.is_admissible());
    let counts = report.gate_counts();
    assert_eq!((counts.unmeasured, counts.not_applicable), (1, 1));
    let rendered = scorecard::render(&report);
    let headline = rendered.lines().nth(1).unwrap();
    assert!(headline.contains("Certified"));
    assert!(headline.contains("2 absent by declaration"));
    assert!(!headline.contains("unmeasured"));
    assert!(headline.contains(&format!(
        "{}/{} gates passed",
        counts.total() - 2,
        counts.total()
    )));
    assert!(rendered.contains("capability absent"));
    assert!(rendered.contains("no applicable input in this change"));
}

#[test]
fn permitted_unmeasured_alone_remains_an_absence_not_a_pass() {
    let report = report(&[("slo_status", unmeasured("slo_status"))]);
    assert!(report.is_admissible());
    let counts = report.gate_counts();
    assert_eq!((counts.unmeasured, counts.not_applicable), (1, 0));
    let rendered = scorecard::render(&report);
    assert!(rendered.contains("1 absent by declaration"));
    assert!(rendered.contains(&format!(
        "{}/{} gates passed",
        counts.total() - 1,
        counts.total()
    )));
}

#[test]
fn empty_subject_alone_remains_separate_from_missing_measurement() {
    let report = report(&[("trace_status", inapplicable("trace_status"))]);
    assert!(report.is_admissible());
    let counts = report.gate_counts();
    assert_eq!((counts.unmeasured, counts.not_applicable), (0, 1));
    let rendered = scorecard::render(&report);
    assert!(rendered.contains("1 gate absent by declaration"));
    assert!(rendered.contains(&format!(
        "{}/{} gates passed",
        counts.total() - 1,
        counts.total()
    )));
}

#[test]
fn blocking_absence_still_blocks_with_an_inapplicable_peer() {
    let report = report(&[
        (
            "formal_verification_status",
            unmeasured("formal_verification_status"),
        ),
        ("trace_status", inapplicable("trace_status")),
    ]);
    assert!(!report.is_admissible());
    let rendered = scorecard::render(&report);
    assert!(rendered.contains("Blocked"));
    assert!(rendered.contains("**formal-verification** — not measured"));
    assert!(rendered.contains("1 gate absent by declaration"));
}
