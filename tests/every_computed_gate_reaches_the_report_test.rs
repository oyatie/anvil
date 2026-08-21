//! A gate that is computed but never carried into the report does not gate.
//!
//! `review_verdict_status` — the AI code review and 16-lens verdict — was
//! computed in the evaluator and never became a field. `all_statuses()` could
//! not see it, `seal()` could not gate on it, and a pull request whose review
//! returned REQUEST_CHANGES or REJECT was certified anyway.
//!
//! It had worked once. When certification moved from a boolean chain into
//! `seal()` — which derives the verdict from every *field* — the value was left
//! behind rather than carried across. Nothing failed at the time. The gate did
//! not error; it stopped mattering, which is a failure mode with no symptom.
//!
//! An unused-variable lint eventually found it, and only because the crate
//! moved to edition 2024. That is far too much luck. This checks the property
//! directly.

use std::collections::BTreeSet;
use std::fs;

fn read(p: &str) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("{p} must be readable: {e}"))
}

/// `let <name>_status` bindings in the evaluator, ignoring comments.
fn computed_in_evaluator() -> BTreeSet<String> {
    let src = read("src/pre_merge_guard/evaluator.rs");
    let mut out = BTreeSet::new();
    for line in src.lines() {
        let t = line.trim_start();
        if t.starts_with("//") {
            continue;
        }
        if let Some(rest) = t.strip_prefix("let ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_')
                .collect();
            if name.ends_with("_status") {
                out.insert(name);
            }
        }
    }
    out
}

/// Every `<name>_status` mentioned anywhere in the evaluator's report literal,
/// which covers both `foo_status,` shorthand and `foo_status: bar_status`.
fn carried_into_the_report() -> BTreeSet<String> {
    let src = read("src/pre_merge_guard/evaluator.rs");
    let start = src
        .find("let mut report = PreMergeCertificationReport {")
        .expect("the report literal must exist");
    let body = &src[start..];
    let end = body.find("\n        };").unwrap_or(body.len());
    let literal = &body[..end];

    let mut out = BTreeSet::new();
    let mut rest = literal;
    while let Some(i) = rest.find("_status") {
        let head = &rest[..i];
        let name_start = head
            .rfind(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
            .map(|x| x + 1)
            .unwrap_or(0);
        out.insert(format!("{}_status", &head[name_start..]));
        rest = &rest[i + 7..];
    }
    out
}

#[test]
fn every_computed_gate_status_is_carried_into_the_report() {
    let computed = computed_in_evaluator();
    let carried = carried_into_the_report();

    assert!(
        !computed.is_empty() && !carried.is_empty(),
        "the scan found nothing; it has stopped measuring rather than passing \
         (computed={}, carried={})",
        computed.len(),
        carried.len()
    );

    let orphans: Vec<&String> = computed.difference(&carried).collect();
    assert!(
        orphans.is_empty(),
        "{} gate verdict(s) are computed and then dropped, so they gate nothing: {:?}\n\
         A gate that is not carried into the report is invisible to all_statuses() and \
         to seal(). It does not fail — it stops mattering, silently.",
        orphans.len(),
        orphans
    );
}

#[test]
fn the_declared_total_matches_what_the_report_actually_carries() {
    let rp = read("src/pre_merge_guard/report.rs");
    // Scope to the struct body: a `: GateStatus,` in a function signature or a
    // doc example elsewhere in the file is not a gate.
    let start = rp
        .find("pub struct PreMergeCertificationReport")
        .expect("struct declaration");
    let body = &rp[start..];
    let end = body.find("\n}\n").expect("struct terminator");
    let fields = body[..end].matches(": GateStatus,").count();
    assert_eq!(
        fields,
        anvil::pre_merge_guard::report::TOTAL_GATES,
        "TOTAL_GATES is published onto pull requests; it must equal the number of gate \
         fields the report carries"
    );
}
