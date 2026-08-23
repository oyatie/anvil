//! A gate's own verdict must reach the report unchanged.
//!
//! `evaluator.rs` rebuilt `GateStatus` from each report's boolean instead of
//! reading `report.status`. That silently undid the honesty work in the guards:
//!
//!   - `slo_report.is_compliant` is true when nothing was measured, so absent
//!     evidence was published as `Passed` -- the inversion I1 forbids.
//!   - `coverage_report.estimated_diff_coverage_percent` is `f64::NAN` when
//!     unmeasured, so `{:.1}` produced the accusation "Coverage NaN% is below
//!     requirement" on every PR adding code without coverage evidence.
//!
//! Both bugs live in the *wiring*, not the guards, so guard-level tests could
//! not see them. This scans the wiring itself.

use std::fs;

fn evaluator_source() -> String {
    let text =
        fs::read_to_string("src/pre_merge_guard/evaluator.rs").expect("evaluator.rs must exist");
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Gates that own a `GateStatus` and can therefore report `NotMeasured`.
const GATES_OWNING_A_VERDICT: &[&str] = &[
    "slo_report",
    "cluster_audit_report",
    "ci_wallclock_report",
    "remote_cache_report",
    "shadow_traffic_report",
    "cosign_report",
];

#[test]
fn gates_that_own_a_verdict_have_it_read_not_rebuilt() {
    let src = evaluator_source();
    let rebuilt: Vec<&str> = GATES_OWNING_A_VERDICT
        .iter()
        .copied()
        .filter(|g| src.contains(&format!("= if {}.is_", g)))
        .collect();

    assert!(
        rebuilt.is_empty(),
        "these reports carry their own GateStatus but the evaluator rebuilds it \
         from a boolean, discarding NotMeasured: {:?}. Use `report.status.clone()`.",
        rebuilt
    );
}

#[test]
fn coverage_verdict_comes_from_the_gate_not_from_a_formatted_float() {
    let src = evaluator_source();

    assert!(
        src.contains("coverage_report.gate_status()"),
        "the evaluator must call CoverageReport::gate_status(), which distinguishes \
         Measured / NotMeasured / NothingToMeasure"
    );
    assert!(
        !src.contains("Coverage {:.1}% is below requirement"),
        "formatting estimated_diff_coverage_percent produces \"Coverage NaN% is below \
         requirement\" when nothing was measured -- a fabricated accusation"
    );
}

#[test]
fn no_gate_verdict_is_synthesised_from_is_compliant() {
    let src = evaluator_source();
    assert!(
        !src.contains("= if slo_report.is_compliant"),
        "slo_report.is_compliant is true when unmeasured; rebuilding from it \
         publishes absent evidence as Passed"
    );
}
