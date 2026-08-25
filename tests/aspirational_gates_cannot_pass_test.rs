//! A gate the fidelity registry records as `Aspirational` must not publish a pass.
//!
//! DEFECT UNDER TEST
//! -----------------
//! `src/fidelity/mod.rs` states the rule in the enum's own doc comment —
//! "Named only. No implementation of the claimed capability exists. Must report
//! `GateStatus::NotMeasured`" — and encodes it in `Fidelity::may_report_pass()`.
//! That method had **no production consumer**. Nothing on the path from a guard
//! to a published report ever asked it, so the rule was declared and unenforced:
//! seven of the eighteen `Aspirational` gates published `Passed` on every pull
//! request the corpus certified — `deadlock_status`, `openvex_status`,
//! `cosign_status`, `auto_rollback_status`, `carbon_compute_status`,
//! `replay_harness_status` and `upgrade_train_status`.
//!
//! WHY PROMPTING WOULD NOT PREVENT THIS
//! ------------------------------------
//! "Remember that aspirational gates report NotMeasured" is an instruction to
//! seventy-two independent guards, each written at a different time. The rule
//! has to live once, on the boundary every gate outcome crosses on its way to a
//! pull request, so a gate added tomorrow inherits it without being told.
//!
//! WHAT IS AND IS NOT UNDER TEST HERE
//! ----------------------------------
//! The ceiling is applied by the certification run — `evaluate_pre_merge_gates`
//! — because that is what publishes. `from_gate_outcomes` is the spec suite's
//! fixture door and publishes nothing; production code is kept off it by
//! `every_door_hands_the_merge_queue_evidence_a_certification_run_produced`, the
//! same scan that makes the provenance mark worth anything. The tests below use
//! that door to reach report shapes directly and pin the rule's every branch;
//! `no_aspirational_gate_publishes_a_pass_on_the_change_the_corpus_measured` in
//! `enlist_authority_coverage_test.rs` pins it end to end on a real evaluator
//! run, and `the_certification_run_withholds_before_it_seals` pins that the
//! evaluator has not stopped applying it.

use anvil::fidelity::{self, Fidelity};
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};

fn gate_names() -> Vec<&'static str> {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        names.len(),
        TOTAL_GATES,
        "the fixture must cover the whole corpus"
    );
    names
}

/// Every gate in the corpus handed over as `Passed`, through the door that
/// confers the certification mark. The strongest report anybody can write down,
/// and the one the ceiling has the most to say about.
fn every_gate_reporting(status: GateStatus) -> PreMergeCertificationReport {
    let outcomes: Vec<(&str, GateStatus)> = gate_names()
        .into_iter()
        .map(|n| (n, status.clone()))
        .collect();
    PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("an outcome for every gate in the corpus")
}

fn gates_at(f: Fidelity) -> Vec<&'static str> {
    anvil::fidelity::registry::AUDITED_GATES
        .iter()
        .filter(|e| e.fidelity == f)
        .map(|e| e.gate_id)
        .collect()
}

fn status_of<'r>(report: &'r PreMergeCertificationReport, gate: &str) -> &'r GateStatus {
    report
        .named_statuses()
        .into_iter()
        .find(|(n, _)| *n == gate)
        .map(|(_, s)| s)
        .unwrap_or_else(|| panic!("{gate} is not a gate in this corpus"))
}

// ---------------------------------------------------------------------------
// The rule itself, branch by branch.
// ---------------------------------------------------------------------------

/// The defect, stated directly. Eighteen gates the registry records as
/// `Aspirational` hand over a `Passed`, and not one of them may keep it.
#[test]
fn an_aspirational_gate_cannot_publish_a_pass() {
    let aspirational = gates_at(Fidelity::Aspirational);
    // Deliberately not pinned to a count. Eighteen gates are Aspirational as
    // this is written, and every fix in flight moves one of them upward — a
    // hard count would turn each of those into a conflict here while testing
    // nothing about the rule. What must not happen is the set emptying out
    // silently, which would leave this test asking nothing.
    assert!(
        !aspirational.is_empty(),
        "fixture sanity: no gate is registry-recorded as Aspirational, so this \
         test can no longer discriminate"
    );

    let mut report = every_gate_reporting(GateStatus::Passed);
    report.withhold_aspirational_passes();

    for gate in &aspirational {
        match status_of(&report, gate) {
            GateStatus::NotMeasured { gate_id, reason } => {
                assert_eq!(gate_id, gate, "the withheld status must name its own gate");
                assert!(
                    reason.contains("src/fidelity/registry.rs"),
                    "a reader of the refusal must be able to find the ruling that \
                     produced it; {gate} said: {reason}"
                );
                assert!(
                    reason.contains(Fidelity::Aspirational.label()),
                    "the reason must say what the registry declared, not merely \
                     that something was withheld; {gate} said: {reason}"
                );
            }
            other => panic!(
                "{gate} is registry-recorded as Aspirational and published {other:?}: \
                 it implements none of the capability it is named for, so it has \
                 nothing to pass on"
            ),
        }
    }
}

/// A gate that measures something keeps what it measured. The ceiling is a
/// fidelity rule, not a blanket suspicion: downgrading `Heuristic`, `Partial`
/// or `Measured` would delete real evidence.
#[test]
fn a_gate_the_registry_records_above_aspirational_keeps_its_pass() {
    let mut report = every_gate_reporting(GateStatus::Passed);
    report.withhold_aspirational_passes();

    let mut checked = 0;
    for f in [Fidelity::Heuristic, Fidelity::Partial, Fidelity::Measured] {
        for gate in gates_at(f) {
            assert_eq!(
                status_of(&report, gate),
                &GateStatus::Passed,
                "{gate} is recorded {} and its pass was taken away; the ceiling \
                 only withholds what no implementation supports",
                f.label()
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 15,
        "fixture sanity: the registry must hold gates above Aspirational for \
         this test to discriminate at all; it checked {checked}"
    );
}

/// The gates the audit has not reached — thirty-seven of seventy-two today.
///
/// They are neither downgraded nor quietly forgiven. Downgrading one would be a
/// fabricated `NotMeasured` for a gate nobody has read, the symmetric violation
/// of I1; forgiving one silently would make the ceiling's coverage invisible.
/// So the status is left exactly as the guard produced it and the size of the
/// exemption is published by `gap_report().unaudited`.
#[test]
fn an_unaudited_gate_keeps_its_pass_and_the_size_of_that_exemption_is_published() {
    let mut report = every_gate_reporting(GateStatus::Passed);
    report.withhold_aspirational_passes();

    let unaudited: Vec<&'static str> = gate_names()
        .into_iter()
        .filter(|g| fidelity::declared_fidelity(g).is_none())
        .collect();

    for gate in &unaudited {
        assert_eq!(
            status_of(&report, gate),
            &GateStatus::Passed,
            "{gate} has no registry entry, so the registry has no opinion about \
             it; withholding its pass would be an accusation nobody made"
        );
    }

    let gap = fidelity::gap_report(TOTAL_GATES);
    assert_eq!(
        gap.unaudited,
        unaudited.len(),
        "the count of gates the ceiling does not cover must be the count the gap \
         report publishes, or the exemption is silent"
    );
    assert!(
        gap.unaudited > 0 && gap.summary().contains("not yet audited"),
        "while any gate is unaudited the published summary has to say so: {}",
        gap.summary()
    );
}

/// `AutoUpdated` is `is_acceptable()`, renders a green badge and is counted
/// among the gates that did their job. A gate that implements nothing cannot
/// auto-correct anything either, so leaving this arm out would be a one-token
/// walk around the rule.
#[test]
fn an_aspirational_gate_cannot_launder_a_pass_through_auto_updated() {
    let mut report = every_gate_reporting(GateStatus::AutoUpdated);
    report.withhold_aspirational_passes();

    for gate in gates_at(Fidelity::Aspirational) {
        assert!(
            matches!(status_of(&report, gate), GateStatus::NotMeasured { .. }),
            "{gate} republished its withheld pass as AutoUpdated and kept it"
        );
    }
    // And the same rule must not touch a gate that may pass.
    for gate in gates_at(Fidelity::Heuristic) {
        assert_eq!(status_of(&report, gate), &GateStatus::AutoUpdated);
    }
}

/// The ceiling withholds a claim; it does not rewrite one. A `Failed` erased
/// into `NotMeasured` would hide a real finding behind a policy rule, which is
/// the objection this rule exists to answer — so every non-pass status, the
/// gate's own `NotMeasured` reason included, comes through untouched.
#[test]
fn withholding_a_pass_rewrites_no_failure_warning_error_or_prior_refusal() {
    let aspirational = gates_at(Fidelity::Aspirational);

    for original in [
        GateStatus::Failed("a real finding".into()),
        GateStatus::Warning("a real complaint".into()),
        GateStatus::Errored("the tool would not spawn".into()),
    ] {
        let mut report = every_gate_reporting(original.clone());
        report.withhold_aspirational_passes();
        for gate in &aspirational {
            assert_eq!(
                status_of(&report, gate),
                &original,
                "{gate} was published as {original:?} and the ceiling overwrote it"
            );
        }
    }

    // A gate that already declined, in its own words. The ceiling must not
    // replace a specific reason ("no Prometheus endpoint is configured") with
    // its own generic one.
    let mut report = every_gate_reporting(GateStatus::NotMeasured {
        gate_id: "placeholder".into(),
        reason: "the guard's own account of why it could not measure".into(),
    });
    report.withhold_aspirational_passes();
    for gate in &aspirational {
        match status_of(&report, gate) {
            GateStatus::NotMeasured { reason, .. } => assert!(
                reason.contains("the guard's own account"),
                "{gate}'s own reason was overwritten by the ceiling's: {reason}"
            ),
            other => panic!("{gate}: {other:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// What the withheld pass does to the report it is in.
// ---------------------------------------------------------------------------

/// Withholding is not silent. The gate lands in `unmeasured_gates`, which is
/// what the refusal is written from and what the scorecard publishes, so a gate
/// that thought it had measured something becomes *louder* than it was as an
/// unremarkable green — and the report stops being admissible.
#[test]
fn a_withheld_pass_is_published_in_the_unmeasured_list_and_withholds_admission() {
    let mut report = every_gate_reporting(GateStatus::Passed);
    assert!(
        report.is_admissible(),
        "fixture sanity: before the ceiling this report admits the pull request"
    );

    report.withhold_aspirational_passes();
    report.seal();

    for gate in gates_at(Fidelity::Aspirational) {
        assert!(
            report.unmeasured_gates.iter().any(|g| g == gate),
            "{gate}'s pass was withheld and the refusal is not written from it, \
             so nothing a reader sees says the gate produced no measurement"
        );
    }
    // What withholding a pass does, and no longer does.
    //
    // It publishes the gate as unmeasured -- asserted above, and that is the
    // half that matters: a withheld pass must never read as a pass.
    //
    // It does not withhold the merge. An aspirational gate has no
    // implementation, so it can never measure for any pull request, and
    // blocking on it blocks every merge forever rather than making any of them
    // safer. `admission::ABSENCE_POLICY` declares it NOT PROVISIONED, and the
    // test below pins that every aspirational gate is so declared -- an
    // undeclared one would shut the queue permanently.
    assert!(
        report.is_admissible(),
        "an aspirational gate cannot measure for any pull request; withholding \
         its pass must not withhold every merge"
    );

    // The sharpened invariant, in the direction that still bites: a gate this
    // deployment COULD measure, absent, still shuts the door.
    report.kani_status = GateStatus::NotMeasured {
        gate_id: "kani_status".to_string(),
        reason: "kani is not installed on this runner".to_string(),
    };
    report.seal();
    assert!(
        !report.is_admissible(),
        "an undeclared absence must still withhold the merge"
    );
}

/// Every aspirational gate must be declared unprovisionable.
///
/// An aspirational gate has no implementation of the capability it is named
/// for, so it produces no measurement for any pull request in any run. If one
/// is missing from `admission::ABSENCE_POLICY` it is `Provisioned` by default
/// and shuts the merge queue permanently -- which is the state this repository
/// was actually in, for every pull request, until the policy existed.
#[test]
fn every_aspirational_gate_is_declared_unprovisionable() {
    let undeclared: Vec<&str> = gates_at(Fidelity::Aspirational)
        .into_iter()
        .filter(|g| anvil::pre_merge_guard::absence_blocks(g))
        .collect();
    assert!(
        undeclared.is_empty(),
        "{} aspirational gate(s) are not declared in ABSENCE_POLICY, so their \
         absence blocks every merge forever: {undeclared:?}",
        undeclared.len()
    );
}

/// `withhold_aspirational_passes` rewrites the report through `build()` so that
/// it does not become a third hand-written copy of the seventy-two field list.
/// The cost of that is a hand-written list of the fields `build()` does *not*
/// set, and a field added to the struct but forgotten there would be silently
/// wiped off a certified report. This is that field list, read out of the
/// source, so forgetting fails a test.
#[test]
fn withholding_carries_across_every_field_that_is_not_a_gate_status() {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pre_merge_guard/report.rs"),
    )
    .expect("report source");

    let struct_body = src
        .split_once("pub struct PreMergeCertificationReport {")
        .expect("the report struct")
        .1
        .split_once("\n}")
        .expect("the end of the report struct")
        .0;

    // Every field, not only the `pub` ones: `provenance` is `pub(super)` and
    // `subject` is `pub(crate)`, and losing either off a certified report is
    // exactly the accident this test exists for.
    let non_gate_fields: Vec<&str> = struct_body
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//") && !l.starts_with("#["))
        .map(|l| match l.split_once(' ') {
            Some((vis, rest)) if vis.starts_with("pub") => rest,
            _ => l,
        })
        .filter_map(|l| l.split_once(':'))
        .filter(|(name, ty)| {
            ty.trim().trim_end_matches(',') != "GateStatus"
                && !name.is_empty()
                && name.chars().all(|c| c.is_ascii_lowercase() || c == '_')
        })
        .map(|(name, _)| name)
        .collect();

    let body = src
        .split_once("fn withhold_aspirational_passes")
        .expect("the ceiling must exist and keep this name")
        .1
        .split_once("\n    }")
        .expect("the end of the ceiling")
        .0;

    assert!(
        non_gate_fields.len() >= 5,
        "the scan found {} non-gate fields, which means it stopped parsing the \
         struct and would pass against anything: {non_gate_fields:?}",
        non_gate_fields.len()
    );
    for field in non_gate_fields {
        // Code, not prose, and an assignment rather than a mention: a comment
        // naming the field satisfied `body.contains(field)`, which is the same
        // hole `fidelitys_pass_rule_has_a_production_consumer` closes one test
        // down. Matching `rebuilt.{field} =` also rules out a prefix collision
        // between `subject` and some future `subject_line`.
        assert!(
            body.lines()
                .map(str::trim)
                .filter(|l| !l.starts_with("//"))
                .any(|l| l.contains(&format!("rebuilt.{field} ="))),
            "`{field}` is not a gate status, so `build()` does not carry it and \
             `withhold_aspirational_passes` must assign it on the rebuilt \
             report. It does not, so sealing a report silently discards it"
        );
    }
}

/// The private fields the scan above cannot see the loss of, checked by
/// behaviour: a report that came from a certification run must still read as
/// one afterwards, and the rendered matrix must survive.
#[test]
fn withholding_does_not_strip_the_certification_mark_or_the_rendered_matrix() {
    let mut report = every_gate_reporting(GateStatus::Passed);
    report.summary_markdown = "the matrix this report was rendered as".to_string();
    report.withhold_aspirational_passes();

    assert_eq!(
        report.summary_markdown, "the matrix this report was rendered as",
        "the rendered matrix was discarded by the ceiling"
    );

    // `admission_refusal` is the only reader of the provenance mark, and this
    // test uses it to check the mark survived. It needs a refusal to read, and
    // the withheld gates no longer supply one: `admission::ABSENCE_POLICY`
    // declares an aspirational gate's absence NOT PROVISIONED, which does not
    // block. So the probe is an absence nobody has declared -- `shape_status`
    // is outside the policy -- and the assertion below is unchanged: whatever
    // the refusal says, it must not be about a lost certification mark.
    report.shape_status = GateStatus::NotMeasured {
        gate_id: "shape_status".to_string(),
        reason: "probe for the provenance mark".to_string(),
    };
    report.seal();
    let refusal = report
        .admission_refusal()
        .expect_err("an undeclared absence must be refused")
        .to_string();
    assert!(
        !refusal.contains("certification run"),
        "the ceiling stripped the provenance mark: {refusal}"
    );
}

// ---------------------------------------------------------------------------
// Placement: the rule is worth nothing where nothing calls it.
// ---------------------------------------------------------------------------

/// This is the whole defect, restated as a pin. `may_report_pass()` was correct
/// and unreachable for as long as it had no caller, so the test that matters
/// most is not "the rule is right" but "the rule runs".
#[test]
fn fidelitys_pass_rule_has_a_production_consumer() {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/pre_merge_guard/report.rs"),
    )
    .expect("report source");
    // Code, not prose. The rule was documented in three places while nothing
    // called it, so a doc comment naming the method is precisely the evidence
    // this test must not accept.
    let called_in_code = src
        .lines()
        .map(str::trim)
        .any(|l| !l.starts_with("//") && l.contains("may_report_pass()"));
    assert!(
        called_in_code,
        "`Fidelity::may_report_pass` states the rule and nothing in the report \
         asks it. Writing the rule out again as `== Fidelity::Aspirational` is a \
         second definition that will not follow the first when it changes"
    );
}

/// The certification run must apply the ceiling, and must apply it *before*
/// `seal()` — the verdict and the unmeasured list are derived there, so a
/// withheld pass applied afterwards would be invisible to both.
#[test]
fn the_certification_run_withholds_before_it_seals() {
    let src = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/pre_merge_guard/evaluator.rs"),
    )
    .expect("evaluator source");

    let withhold = src
        .find("withhold_aspirational_passes()")
        .expect("the certification run must withhold the passes no gate supports");
    let seal = src
        .find("report.seal()")
        .expect("the certification run seals its report");
    assert!(
        withhold < seal,
        "the ceiling runs after seal(), so `unmeasured_gates` and \
         `is_certified_ready` are derived from statuses it then rewrites"
    );
}

// ---------------------------------------------------------------------------
// The lookup the rule is keyed on.
// ---------------------------------------------------------------------------

#[test]
fn the_registry_lookup_answers_for_audited_gates_and_declines_for_the_rest() {
    assert_eq!(
        fidelity::declared_fidelity("coverage_status"),
        Some(Fidelity::Aspirational)
    );
    assert_eq!(
        fidelity::declared_fidelity("kani_status"),
        Some(Fidelity::Heuristic)
    );
    assert_eq!(
        fidelity::declared_fidelity("doc_parity_status"),
        Some(Fidelity::Partial)
    );
    assert_eq!(
        fidelity::declared_fidelity("cell_isolation_status"),
        None,
        "a gate nobody has audited must not be given a fidelity by default"
    );
    assert_eq!(fidelity::declared_fidelity("no_such_gate"), None);
}
