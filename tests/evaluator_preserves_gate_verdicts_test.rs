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

/// The gate id a report's verdict is published under: `cosign_report` ->
/// `cosign_status`, uniformly for all six.
fn gate_id_of(report: &str) -> String {
    report.replace("_report", "_status")
}

/// Catches: every rebuild shape, not the two that happened to be written first.
///
/// This was a *negative* check for `= if {report}.is_`, and a verdict rebuilt as
/// `= if cosign_report.status.is_acceptable()` walks straight through that -- a
/// hole demonstrated by exploitation, with all six gates exposed and only the
/// gate whose author noticed protected by a bespoke assertion. Requiring the one
/// line that is correct closes it for every shape at once: any rebuild, from a
/// boolean or from the status itself, leaves the expected line absent.
#[test]
fn gates_that_own_a_verdict_have_it_read_not_rebuilt() {
    let src = evaluator_source();
    let missing: Vec<String> = GATES_OWNING_A_VERDICT
        .iter()
        .map(|g| format!("let {} = {g}.status.clone();", gate_id_of(g)))
        .filter(|line| !src.contains(line))
        .collect();

    assert!(
        missing.is_empty(),
        "these gates own their GateStatus and the evaluator must carry it \
         through verbatim; the expected line is not in evaluator.rs, so the \
         verdict is being rebuilt from something else and NotMeasured is \
         discarded: {missing:?}"
    );
}

/// Catches: the other end of the same wire. An evaluator that mints a gate's
/// `NotMeasured` itself publishes absence for ever, including after the guard
/// starts producing a real verdict. Absence is the guard's finding, not the
/// wiring's opinion.
#[test]
fn the_evaluator_mints_no_verdict_for_a_gate_that_owns_one() {
    let src = evaluator_source();
    let minted: Vec<String> = GATES_OWNING_A_VERDICT
        .iter()
        .map(|g| format!("gate_id: \"{}\"", gate_id_of(g)))
        .filter(|needle| src.contains(needle))
        .collect();

    assert!(
        minted.is_empty(),
        "the evaluator mints a status for a gate whose guard owns one, so a \
         real measurement can never reach the report: {minted:?}"
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
