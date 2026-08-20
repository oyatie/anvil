//! The certification verdict must be derived from every gate in the report —
//! by construction, not by a hand-maintained list.
//!
//! DEFECT UNDER TEST
//! -----------------
//! `evaluate_pre_merge_gates` held a 68-term `is_certified_ready = a && b && …`
//! conjunction computed *before* `brand_absence_status` and
//! `migration_boundary_status` were evaluated. Both gates were added to the
//! 70-field report, pinned by `TOTAL_GATES`, rendered nowhere in the matrix,
//! and absent from the verdict: a failing self-directed gate still produced
//! "✅ Certified". Any gate added the same way would inherit the same hole.
//!
//! WHY PROMPTING WOULD NOT PREVENT THIS
//! ------------------------------------
//! "Add the new gate to the conjunction" is a second copy of the field list,
//! 70 lines from the first. Nothing in the compiler notices a term missing
//! from an `&&` chain. The fix is structural — the verdict reads
//! `all_statuses()` — and this test pins the structure so the chain cannot
//! come back.

use std::fs;
use std::path::PathBuf;

fn evaluator_source() -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pre_merge_guard/evaluator.rs");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn the_verdict_is_sealed_from_the_report_not_precomputed() {
    let src = evaluator_source();
    assert!(
        src.contains("report.seal()"),
        "evaluator must derive the verdict with PreMergeCertificationReport::seal()"
    );
    let literal = src
        .find("PreMergeCertificationReport {")
        .expect("evaluator constructs the report");
    let after = &src[literal..];
    assert!(
        after.contains("is_certified_ready: false"),
        "the report literal must not carry a caller-computed verdict"
    );
}

#[test]
fn no_hand_written_conjunction_of_gate_statuses_remains() {
    let src = evaluator_source();
    let chained: Vec<&str> = src
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            t.starts_with("&&") && t.contains("_status.is_acceptable()")
        })
        .collect();
    assert!(
        chained.is_empty(),
        "a hand-written `&& <gate>_status.is_acceptable()` chain is a second copy of the \
         field list and will miss the next gate: {chained:?}"
    );
}

#[test]
fn no_gate_status_is_computed_after_the_report_is_constructed() {
    let src = evaluator_source();
    let literal_line = src
        .lines()
        .position(|l| l.contains("PreMergeCertificationReport {"))
        .expect("evaluator constructs the report");
    let late: Vec<String> = src
        .lines()
        .enumerate()
        .skip(literal_line)
        .filter(|(_, l)| {
            let t = l.trim_start();
            t.starts_with("let ") && t.contains("_status") && t.contains('=')
        })
        .map(|(i, l)| format!("{}: {}", i + 1, l.trim()))
        .collect();
    assert!(
        late.is_empty(),
        "gate statuses bound after the report literal are invisible to the verdict \
         unless seal() runs after them; keep every status above the literal: {late:?}"
    );
}
