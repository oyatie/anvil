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
    // Four marker-scoped gates that now own a GateStatus so an empty scope
    // reports NotMeasured instead of a pass. Without them here, restoring
    // `= if debt_shrink_report.is_acceptable` passes CI and the change is
    // undone in the wiring, where the gate-level tests cannot see it.
    //
    // The evaluator's parameter was `debt_report` while its status is
    // `debt_shrink_status`, which breaks the name rule this list is checked
    // against. Renamed to `debt_shrink_report` rather than exempting the gate.
    "debt_shrink_report",
    "ghost_migration_report",
    "gitops_drift_report",
    "migration_orch_report",
    "mutation_report",
    // `is_idiomatic` is true both for a clean scan and for a diff with no `.rs`
    // file, so rebuilding the verdict from it published "380 rules evaluated,
    // compliant" over zero Rust files. The guard now tells the two apart and
    // the evaluator must carry its answer through.
    "rust_skills_report",
    // The receipt stamper. Its verdict was rebuilt here from an `is_attested`
    // boolean whose only production value was the literal `true`, so the gate
    // passed on every pull request and the `Failed` arm the evaluator wrote for
    // it could not be reached. Without this entry, restoring that rebuild
    // passes CI.
    "attestation_report",
];

/// The gate id a report's verdict is published under: `cosign_report` ->
/// `cosign_status` for all but one, where the report and the gate are not
/// named the same thing. The exception is listed, not derived: a gate whose id
/// diverges and is *not* listed here makes both checks below look for a line
/// that cannot exist, which is red, not silence.
fn gate_id_of(report: &str) -> String {
    match report {
        "debt_report" => "debt_shrink_status".to_string(),
        _ => report.replace("_report", "_status"),
    }
}

/// The two ways an evaluator may obtain a gate's verdict, neither of which
/// authors one: `status.clone()` copies the field the guard set, and
/// `gate_status()` calls a method the guard's own crate implements. A gate that
/// must tell three outcomes apart -- measured-pass, measured-failure, and
/// nothing measured -- has no single `status` field to clone, so it publishes
/// the method instead; the decision is the gate's either way. Anything else, a
/// boolean widened by an `if` or a `match` written in the wiring, is the
/// evaluator deciding, which is what this file exists to forbid.
fn reads_the_gates_verdict(src: &str, report: &str) -> bool {
    let id = gate_id_of(report);
    src.contains(&format!("let {id} = {report}.status.clone();"))
        || src.contains(&format!("let {id} = {report}.gate_status();"))
}

/// Catches: every rebuild shape, not the two that happened to be written first.
///
/// This was a *negative* check for `= if {report}.is_`, and a verdict rebuilt as
/// `= if cosign_report.status.is_acceptable()` walks straight through that -- a
/// hole demonstrated by exploitation, with all six gates exposed and only the
/// gate whose author noticed protected by a bespoke assertion. Requiring one of
/// the two lines that are correct closes it for every shape at once: any
/// rebuild, from a boolean or from the status itself, leaves both absent.
#[test]
fn gates_that_own_a_verdict_have_it_read_not_rebuilt() {
    let src = evaluator_source();
    let missing: Vec<&&str> = GATES_OWNING_A_VERDICT
        .iter()
        .filter(|g| !reads_the_gates_verdict(&src, g))
        .collect();

    assert!(
        missing.is_empty(),
        "these gates own their GateStatus and the evaluator must carry it \
         through verbatim; neither `let <id> = <report>.status.clone();` nor \
         `let <id> = <report>.gate_status();` is in evaluator.rs, so the \
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

/// The same wiring failure, one gate over. `mutation_report.is_adequate` is
/// false BOTH for a mutant the suite failed to kill and for a run that measured
/// nothing at all, so `if is_adequate { Passed } else { Warning }` published
/// absent evidence as an acceptable warning -- and `Warning` is acceptable to
/// `is_admissible`, so the gate could not block whatever it found.
#[test]
fn mutation_verdict_comes_from_the_gate_not_from_a_two_way_boolean() {
    let src = evaluator_source();

    assert!(
        src.contains("mutation_report.gate_status()"),
        "the evaluator must call MutationAdequacyReport::gate_status(), which \
         distinguishes a surviving mutant (Failed) from a run that measured \
         nothing (NotMeasured) from nothing to mutate (Passed)"
    );
    assert!(
        !src.contains("= if mutation_report.is_adequate"),
        "rebuilding the mutation verdict from is_adequate collapses `no mutant \
         survived` and `no mutant ran` into the same answer"
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
