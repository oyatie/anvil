//! Real ratchet verdicts must retain every refusal at the certification seam.
//! Reports, baselines and signoffs are ordinary in-memory data, not Git fixtures.

use anvil::pre_merge_guard::{GateStatus, evaluator::shape_gate_status};
use anvil::ratchet::core::{Baseline, Mode, Signing, Signoff, compare};
use anvil::shape::core::{Finding, RuleId, ShapeReport, SpecSource};
use anvil::shape::facade::baseline::{Judgement, keys_by_rule};
use anvil::shape::facade::gate::{ShapeGateOutcome, outcome_from_judgement};
use std::collections::BTreeSet;

const RULE: &str = "file_misplaced";

fn report(keys: &[&str]) -> ShapeReport {
    ShapeReport {
        repo: "measured-repo".into(),
        rev: "b".repeat(40),
        spec_source: SpecSource::Adopted,
        units: Vec::new(),
        findings: keys
            .iter()
            .map(|key| Finding {
                rule: RuleId::new(RULE),
                key: (*key).into(),
                path: (*key).into(),
                unit: None,
                detail: "ordinary placement finding".into(),
                fix: None,
            })
            .collect(),
        not_measured: Vec::new(),
    }
}

fn project(
    mode: Mode,
    old: &[&str],
    now: &[&str],
    declared: bool,
    signoff: &Signoff,
) -> ShapeGateOutcome {
    let frozen = Baseline::seed(
        &"a".repeat(40),
        &keys_by_rule(&report(old)),
        &[(RULE.into(), (mode, false))].into(),
    );
    let head = report(now);
    let declared_now: BTreeSet<String> = if declared {
        [RULE.into()].into()
    } else {
        BTreeSet::new()
    };
    let verdict = compare(
        &frozen,
        &keys_by_rule(&head),
        signoff,
        |_| None,
        &declared_now,
    );
    let blocking_rules = if mode == Mode::BlockOnNew {
        declared_now
    } else {
        BTreeSet::new()
    };
    // A withdrawn rule really is absent from the head's blocking set. The
    // unmeasured join cannot be used as a substitute for verdict projection.
    outcome_from_judgement(
        Judgement::Judged {
            merge_base: "a".repeat(40),
            report: head,
            verdict,
            blocking_rules,
        },
        "example/repo",
    )
}

fn refusal(outcome: &ShapeGateOutcome, reason: &str, new_keys: usize) {
    let m = outcome.measurement().unwrap();
    assert_eq!(m.repo, "example/repo");
    assert_eq!(m.blocking_regressions, new_keys);
    assert!(m.blocking_unmeasured.is_empty());
    assert!(
        matches!(shape_gate_status(outcome), GateStatus::Failed(msg)
        if msg.contains(RULE) && msg.contains(reason) && msg.contains("1 blocking refusal(s)")),
        "{outcome:?}"
    );
    let summary = outcome.summary();
    assert!(
        summary.contains(&format!("{new_keys} new on blocking rules")),
        "{summary}"
    );
    assert!(summary.contains("1 blocking refusal(s)"), "{summary}");
}

#[test]
fn withdrawn_blocking_debt_remains_a_named_failure_without_fake_new_keys() {
    let outcome = project(
        Mode::BlockOnNew,
        &["old.rs"],
        &[],
        false,
        &Signoff::default(),
    );
    refusal(&outcome, "withdrawn", 0);
    assert_eq!(outcome.measurement().unwrap().fixed, 0);
}

#[test]
fn withdrawn_advisory_debt_does_not_become_blocking_or_fixed() {
    let outcome = project(Mode::Advisory, &["old.rs"], &[], false, &Signoff::default());
    assert_eq!(shape_gate_status(&outcome), GateStatus::Passed);
    assert_eq!(outcome.measurement().unwrap().fixed, 0);
    assert_eq!(outcome.measurement().unwrap().blocking_regressions, 0);
}

#[test]
fn retiring_an_empty_blocking_baseline_has_no_debt_to_refuse() {
    let outcome = project(Mode::BlockOnNew, &[], &[], false, &Signoff::default());
    assert_eq!(shape_gate_status(&outcome), GateStatus::Passed);
    assert_eq!(outcome.measurement().unwrap().fixed, 0);
}

#[test]
fn a_declared_rule_that_is_now_clean_is_really_fixed() {
    let outcome = project(
        Mode::BlockOnNew,
        &["old.rs"],
        &[],
        true,
        &Signoff::default(),
    );
    assert_eq!(shape_gate_status(&outcome), GateStatus::Passed);
    assert_eq!(outcome.measurement().unwrap().fixed, 1);
    assert_eq!(outcome.measurement().unwrap().blocking_regressions, 0);
}

#[test]
fn an_ordinary_new_key_keeps_its_name_and_numeric_regression_count() {
    let outcome = project(
        Mode::BlockOnNew,
        &[],
        &["new.rs"],
        true,
        &Signoff::default(),
    );
    refusal(&outcome, "new.rs", 1);
    assert_eq!(outcome.measurement().unwrap().fixed, 0);
}

#[test]
fn an_inert_signoff_is_a_refusal_not_a_new_measured_key() {
    let mut signoff = Signoff::default();
    signoff
        .additions
        .insert(RULE.into(), ["absent.rs".into()].into());
    signoff.signings.push(Signing {
        by: "owner".into(),
        date: "2026-09-06".into(),
        note: "ordinary signoff data".into(),
    });
    let outcome = project(Mode::BlockOnNew, &[], &[], true, &signoff);
    refusal(&outcome, "inert signoff for absent.rs", 0);
}
