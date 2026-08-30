//! The absence policy may not become a way of switching gates off.
//!
//! `admission_refusal` used to refuse any report containing a gate without a
//! measurement. Correct against the original defect -- absence of a finding
//! read as absence of a problem -- and one step too far: run against this
//! repository's own pull requests, 34 of 72 gates report `NotMeasured`, so no
//! pull request has ever been admissible.
//!
//! A gate that can never pass does not make merges safer. It makes the queue
//! unreachable, and an unreachable queue gets drained by hand -- which is how
//! the corpus stops being consulted at all.
//!
//! `ABSENCE_POLICY` splits absence into three and blocks on one. These tests
//! are what stop the split being a bypass.

use anvil::pre_merge_guard::GateStatus;
use anvil::pre_merge_guard::admission::{ABSENCE_POLICY, Absence, absence_blocks, absence_of};
use anvil::pre_merge_guard::report::PreMergeCertificationReport;
use std::collections::BTreeSet;

fn every_gate_id() -> BTreeSet<&'static str> {
    PreMergeCertificationReport::unmeasured("enumerating")
        .named_statuses()
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

#[test]
fn a_gate_nobody_has_argued_about_still_blocks() {
    // The default is what keeps invariant I1 alive. A gate added tomorrow that
    // fails to measure blocks, without anyone having to remember to say so.
    assert_eq!(
        absence_of("a_gate_that_does_not_exist"),
        Absence::Provisioned
    );
    assert!(absence_blocks("a_gate_that_does_not_exist"));

    // And a real gate outside the table blocks too.
    let listed: BTreeSet<&str> = ABSENCE_POLICY.iter().map(|(id, _)| *id).collect();
    let unlisted: Vec<&str> = every_gate_id()
        .into_iter()
        .filter(|id| !listed.contains(id))
        .collect();
    assert!(
        !unlisted.is_empty(),
        "if every gate were listed, nothing could ever block and the policy \
         would be an off switch rather than a classification"
    );
    assert!(unlisted.iter().all(|id| absence_blocks(id)));
}

#[test]
fn every_row_names_a_real_gate() {
    // A row for a gate that no longer exists is a hole held open for nothing,
    // and the next gate to take that id inherits the exemption silently.
    let real = every_gate_id();
    let stale: Vec<&str> = ABSENCE_POLICY
        .iter()
        .map(|(id, _)| *id)
        .filter(|id| !real.contains(id))
        .collect();
    assert!(
        stale.is_empty(),
        "absence policy names gates that do not exist: {stale:?}"
    );
}

#[test]
fn every_row_says_what_is_missing() {
    // An operator must read a capability or a subject set, not a shrug. This is
    // the difference between a classification and a waiver.
    for (id, absence) in ABSENCE_POLICY {
        let text = match absence {
            Absence::NotProvisioned { capability } => *capability,
            Absence::NotApplicable { subject } => *subject,
            Absence::Provisioned => panic!("{id}: a Provisioned row says nothing; omit it instead"),
        };
        assert!(
            text.len() > 20 && text.contains(' '),
            "{id}: `{text}` does not name what is missing"
        );
    }
}

#[test]
fn no_gate_is_listed_twice() {
    let mut seen = BTreeSet::new();
    for (id, _) in ABSENCE_POLICY {
        assert!(
            seen.insert(*id),
            "{id} is listed twice; one row wins silently"
        );
    }
}

#[test]
fn a_not_applicable_gate_is_still_a_gate_that_ran() {
    // NotApplicable is a claim about the CHANGE, so it must name a subject set
    // a reader can check the change against -- never a capability.
    for (id, absence) in ABSENCE_POLICY {
        if let Absence::NotApplicable { subject } = absence {
            assert!(
                !subject.contains("configured") && !subject.contains("endpoint"),
                "{id}: `{subject}` describes a missing capability, not an empty \
                 subject set; it belongs under NotProvisioned"
            );
        }
    }
}

/// Every gate, at a status of your choosing, through the door that carries the
/// certification mark.
fn report_with(overrides: &[(&str, GateStatus)]) -> PreMergeCertificationReport {
    let outcomes: Vec<(&str, GateStatus)> = every_gate_id()
        .into_iter()
        .map(|id| {
            let status = overrides
                .iter()
                .find(|(o, _)| *o == id)
                .map(|(_, s)| s.clone())
                .unwrap_or(GateStatus::Passed);
            (id, status)
        })
        .collect();
    PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("an outcome for every gate in the corpus")
}

fn not_measured(id: &str) -> GateStatus {
    GateStatus::NotMeasured {
        gate_id: id.to_string(),
        reason: "declared absent for this deployment or this change".to_string(),
    }
}

#[test]
fn an_errored_gate_blocks_whatever_the_policy_says() {
    // `Errored` means the gate had a source and the call failed. That is a
    // defect, and no row in this table may excuse it -- `slo_status` is the
    // most-declared gate in the table and it still blocks when it errors.
    let report = report_with(&[(
        "slo_status",
        GateStatus::Errored("probe crashed".to_string()),
    )]);
    let refusal = format!(
        "{:#}",
        report
            .admission_refusal()
            .expect_err("an Errored gate must never be admitted")
    );
    assert!(
        refusal.contains("slo_status"),
        "the refusal must name the gate: {refusal}"
    );
}

#[test]
fn a_declared_absence_no_longer_withholds_the_merge() {
    // The point of the change, as an assertion. Every gate the policy declares
    // absent reports NotMeasured; everything else passes. Before this, that
    // report was refused and no pull request in this repository was ever
    // admissible.
    let declared: Vec<(&str, GateStatus)> = ABSENCE_POLICY
        .iter()
        .map(|(id, _)| (*id, not_measured(id)))
        .collect();
    let report = report_with(&declared);
    assert!(
        report.admission_refusal().is_ok(),
        "declared absences still withhold the merge: {:#}",
        report.admission_refusal().unwrap_err()
    );
}

#[test]
fn an_undeclared_absence_still_withholds_the_merge() {
    // The other half. One gate outside the table, absent, and the door shuts --
    // otherwise the change would have replaced blocking-on-everything with
    // blocking-on-nothing.
    let listed: BTreeSet<&str> = ABSENCE_POLICY.iter().map(|(id, _)| *id).collect();
    let victim = every_gate_id()
        .into_iter()
        .find(|id| !listed.contains(id))
        .expect("some gate is outside the policy");

    let report = report_with(&[(victim, not_measured(victim))]);
    let refusal = format!(
        "{:#}",
        report
            .admission_refusal()
            .expect_err("an undeclared absence must still be refused")
    );
    assert!(
        refusal.contains(victim),
        "the refusal must name {victim}: {refusal}"
    );
}

// ---------------------------------------------------------------------------
// What a reader sees
// ---------------------------------------------------------------------------

/// The scorecard's own rule 1 -- findings only, the rest counted -- had never
/// been applied to absences.
///
/// Measured on this repository's own PR #121, the posted comment opened:
///
/// ```text
/// ❌ Blocked — 38 finding(s) across 72 gates; 34 gate(s) produced no measurement.
/// ```
///
/// followed by 34 multi-line paragraphs about Prometheus endpoints and Sigstore
/// backends, with the three things a reader could act on buried inside them.
/// That is the same defect as the 68-row table of `PASSED` the rule was written
/// for, wearing absence as a costume.
#[test]
fn the_scorecard_leads_with_what_needs_action() {
    let declared: Vec<(&str, GateStatus)> = ABSENCE_POLICY
        .iter()
        .map(|(id, _)| (*id, not_measured(id)))
        .collect();
    let mut overrides = declared.clone();
    overrides.push((
        "shape_status",
        GateStatus::Failed("root file \"deny.toml\" is not on the allowlist".to_string()),
    ));
    let report = report_with(&overrides);
    let card = anvil::publish::scorecard::render(&report);

    let headline = card
        .lines()
        .find(|l| l.starts_with("❌"))
        .expect("a blocked scorecard leads with the verdict");
    assert!(
        headline.contains("1 finding(s) need action"),
        "the headline must count what a reader can act on, not the corpus: {headline}"
    );

    // The one thing to act on is above the fold; the declared absences are
    // inside it, counted and still present.
    let fold = card
        .find("<details>")
        .expect("declared absences are folded");
    let shape = card.find("shape").expect("the real finding is rendered");
    assert!(
        shape < fold,
        "the actionable finding must come before the fold"
    );
    assert!(
        card.contains(&format!(
            "{} gates absent by declaration",
            ABSENCE_POLICY.len()
        )),
        "the fold must say how many, so nobody has to count them: {card}"
    );
    assert!(
        card.contains("slo"),
        "declared absences are folded, never dropped -- hiding them is how a \
         corpus quietly stops measuring"
    );
}

#[test]
fn the_scorecard_and_the_enlister_agree_on_admissibility() {
    // Two definitions of admissible disagreeing is the defect the doc on
    // `admission_refusal` was written to prevent. `is_admissible` required
    // `unmeasured_gates` to be empty, so the scorecard would have published
    // "Blocked" over a pull request the enlister admits.
    let declared: Vec<(&str, GateStatus)> = ABSENCE_POLICY
        .iter()
        .map(|(id, _)| (*id, not_measured(id)))
        .collect();
    let report = report_with(&declared);
    assert_eq!(
        report.is_admissible(),
        report.admission_refusal().is_ok(),
        "the scorecard and the merge-queue door must ask the same question"
    );
    assert!(report.is_admissible());
}

/// An empty subject set is `NotApplicable`, and `NotApplicable` admits.
///
/// This is the shape of PR #129: zero failing gates, two tree-wide warnings,
/// and two gates reporting an absence -- refused. `formal_verification_status`
/// was reporting "this change adds no line to a policy file" as `NotMeasured`,
/// which withholds admission from every pull request that touches no policy,
/// which is nearly all of them.
///
/// The earlier correction was right and is preserved: a diff carrying no
/// policy must NOT publish a green gate. But `Passed` and `NotMeasured` are
/// not the only two answers. The scan RAN; its subject set was empty. That is
/// `NotApplicable`, and this repository already had the variant.
#[test]
fn a_gate_whose_subject_set_is_empty_does_not_withhold_the_merge() {
    let absent = GateStatus::NotApplicable {
        gate_id: "formal_verification_status".to_string(),
        subject: "this change adds no line to a policy file".to_string(),
    };
    let report = report_with(&[("formal_verification_status", absent)]);
    assert!(
        report.is_admissible(),
        "a gate that ran over an empty subject set must not withhold admission; \
         refusal was: {:?}",
        report.admission_refusal().err()
    );

    // And the distinction is load-bearing, not cosmetic: the same gate, the
    // same change, reported as an absence of MEASUREMENT rather than an
    // absence of SUBJECT, still refuses. If this half ever passes, the two
    // states have been collapsed again and the guarantee is gone.
    let unmeasured = report_with(&[(
        "formal_verification_status",
        not_measured("formal_verification_status"),
    )]);
    assert!(
        !unmeasured.is_admissible(),
        "`NotMeasured` on an unexempted gate must still withhold admission"
    );
}

/// `NotApplicable` is not a pass, and must never be counted as one.
///
/// The whole point of the variant is that it sits between "clean" and "could
/// not look". A reader of the scorecard must be able to tell it from a gate
/// that measured something and found nothing.
#[test]
fn not_applicable_is_admissible_without_being_a_pass() {
    let report = report_with(&[(
        "formal_verification_status",
        GateStatus::NotApplicable {
            gate_id: "formal_verification_status".to_string(),
            subject: "no policy line in this change".to_string(),
        },
    )]);
    let status = report
        .named_statuses()
        .into_iter()
        .find(|(g, _)| *g == "formal_verification_status")
        .map(|(_, s)| s)
        .expect("the gate is in the corpus");
    assert!(
        !matches!(status, GateStatus::Passed),
        "an empty subject set must not be laundered into a pass"
    );
}
