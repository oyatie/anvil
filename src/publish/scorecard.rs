//! Renders the pre-merge scorecard Anvil posts on pull requests.
//!
//! # Style
//!
//! Modelled on how machine reviewers actually post at scale (Google Tricorder /
//! Critique): terse, findings-only, one line per finding, and near-silent on
//! success. The previous rendering emitted a 68-row table in which sixty-odd
//! rows said `PASSED`, burying the two or three that needed action, and each
//! row described what the gate was *for* rather than what had happened.
//!
//! Rules, applied uniformly to every artifact Anvil publishes:
//!
//!   1. Findings only. Passing gates are counted, never enumerated. A reader
//!      scrolling a list of successes learns nothing.
//!   2. One line per finding: `gate — what happened`, then location and fix.
//!   3. Location as `file:line` whenever the gate supplied one.
//!   4. Every finding says what to do next, or says nothing rather than
//!      guessing.
//!   5. Deterministic ordering, so a re-render produces a reviewable diff.
//!   6. One status glyph, at the start. No decorative emoji.
//!   7. Signature last, always.

use crate::fidelity::{Fidelity, registry::AUDITED_GATES};
use crate::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use crate::publish::{AnvilAction, body};

/// Remediation per gate id. Absent where no concrete action is known --
/// invented advice sends the reader somewhere wrong, which is worse than none.
const REMEDIATION: &[(&str, &str)] = &[
    (
        "doc_parity_status",
        "update the affected docs, or add an ADR under docs/decisions/",
    ),
    (
        "modularization_status",
        "split into submodules; ceiling is 300 effective lines",
    ),
    (
        "coverage_status",
        "add tests covering the lines this PR adds",
    ),
    (
        "unresolved_review_status",
        "resolve the open review threads, or reply why they do not apply",
    ),
    (
        "cedar_status",
        "add or widen the Cedar policy covering the new route",
    ),
    (
        "supply_chain_status",
        "remove or replace the flagged dependency",
    ),
    (
        "semantic_abi_status",
        "restore the removed public item, or bump major and note it in CHANGELOG.md",
    ),
    (
        "secret_scan_status",
        "remove the credential and rotate it; history rewriting is not sufficient",
    ),
    (
        "test_suite_status",
        "fix the failing tests locally before pushing",
    ),
    (
        "shape_status",
        "run `anvil shape plan --repo-dir <clone>` for the move plan; a regression on a blocking rule needs an entry in .anvil/baselines/shape.signoff.json",
    ),
];

fn remediation_for(gate_id: &str) -> Option<&'static str> {
    REMEDIATION
        .iter()
        .find(|(g, _)| *g == gate_id)
        .map(|(_, r)| *r)
}

fn fidelity_for(gate_id: &str) -> Option<Fidelity> {
    AUDITED_GATES
        .iter()
        .find(|e| e.gate_id == gate_id)
        .map(|e| e.fidelity)
}

/// `doc_parity_status` -> `doc-parity`. Stable, lowercase, no decoration.
fn gate_name(gate_id: &str) -> String {
    gate_id
        .strip_suffix("_status")
        .unwrap_or(gate_id)
        .replace('_', "-")
}

/// One finding, rendered on a single line plus optional detail lines.
///
/// Carries no fidelity note. The registry records nearly the whole corpus below
/// `Measured`, so a note per finding is one identical sentence on almost every
/// line -- kilobytes of it in the worst case, which is enough to push the terse
/// rendering past the size of the table it exists to replace, and enough to
/// bury the findings a reader can act on. `understatement_note` says it once.
fn finding_line(gate_id: &str, kind: &str, detail: &str) -> String {
    let mut s = format!("- **{}** — {}: {}", gate_name(gate_id), kind, detail.trim());
    if let Some(fix) = remediation_for(gate_id) {
        s.push_str(&format!("\n  - fix: {}", fix));
    }
    s
}

/// Whether the registry records this gate as measuring less than its name says.
fn understates_itself(gate_id: &str) -> bool {
    fidelity_for(gate_id).is_some_and(|f| f < Fidelity::Measured)
}

/// The one line that says which of the findings above come from gates that do
/// not measure what they are named for. Empty when none of them do.
fn understatement_note(gates: &[String]) -> String {
    if gates.is_empty() {
        return String::new();
    }
    format!(
        "\n⚠️ {} of the finding(s) above come from gates that do not fully measure what \
         their names imply: {}. See `src/fidelity/registry.rs` for what each one checks.\n",
        gates.len(),
        gates.join(", ")
    )
}

/// The passing gates the fidelity registry records as `Heuristic` or `Partial`.
///
/// A gate can pass on a keyword scan; the registry is where that is written
/// down. Naming them next to the score is what stops a full-marks total from
/// being read as that many measurements.
///
/// `Aspirational` is excluded rather than merely absent in practice.
/// `withhold_aspirational_passes` turns such a gate's pass into `NotMeasured`
/// before the report is sealed, so it is disclosed on the `unmeasured_gates`
/// path instead; naming it here as well would put one gate on the scorecard
/// under two incompatible descriptions -- "passed, but does not fully measure"
/// and "produced no measurement".
fn low_fidelity_passing_gates(report: &PreMergeCertificationReport) -> Vec<String> {
    report
        .named_statuses()
        .into_iter()
        .filter(|(_, status)| matches!(status, GateStatus::Passed | GateStatus::AutoUpdated))
        .filter(|(gate_id, _)| {
            fidelity_for(gate_id).is_some_and(|f| f.may_report_pass() && f < Fidelity::Measured)
        })
        .map(|(gate_id, _)| gate_name(gate_id))
        .collect()
}

/// The gates that passed on this change, as the proof registry names them.
///
/// Deliberately the raw `gate_id`, not `gate_name`: the registry keys on the
/// id, and translating to a display name here would mean two spellings of the
/// same gate had to agree forever. `low_fidelity_passing_gates` above renders
/// display names because it only ever prints them; this list is matched.
fn passing_gate_ids(report: &PreMergeCertificationReport) -> Vec<String> {
    report
        .named_statuses()
        .into_iter()
        .filter(|(_, status)| matches!(status, GateStatus::Passed | GateStatus::AutoUpdated))
        .map(|(gate_id, _)| gate_id.to_string())
        .collect()
}

/// Renders the scorecard body, signature included.
pub fn render(report: &PreMergeCertificationReport) -> String {
    let counts = report.gate_counts();
    let passed = counts.passed;
    let total = counts.total();

    // Rule 1 -- findings only, the rest counted -- applied to ABSENCES as well
    // as to passes. The rule was written for the 68-row table in which sixty
    // `PASSED` rows buried the two that mattered, and absence had exactly the
    // same effect: 34 "not measured" paragraphs, each several lines long, with
    // the three findings a reader could act on somewhere inside them.
    //
    // An absence `admission::ABSENCE_POLICY` declares is not a finding. Nobody
    // can act on "no Prometheus endpoint is configured" from a pull request,
    // and nobody should have to scroll past it to reach the failure that
    // blocks them. It is counted, named in one folded line, and kept -- because
    // hiding it entirely is how a corpus quietly stops measuring anything.
    let mut findings: Vec<String> = Vec::new();
    let mut declared_absent: Vec<String> = Vec::new();
    let mut understated_findings: Vec<String> = Vec::new();
    for (gate_id, status) in report.named_statuses() {
        let line = match status {
            GateStatus::Failed(r) => Some(finding_line(gate_id, "failed", r)),
            GateStatus::Errored(r) => Some(finding_line(gate_id, "errored", r)),
            GateStatus::NotMeasured { reason, .. } => {
                if crate::pre_merge_guard::absence_blocks(gate_id) {
                    Some(finding_line(gate_id, "not measured", reason))
                } else {
                    declared_absent.push(format!("- **{}** — {reason}", gate_name(gate_id)));
                    None
                }
            }
            GateStatus::Warning(r) => Some(finding_line(gate_id, "warning", r)),
            // The gate ran and its subject set was empty. Folded with the
            // declared absences: nobody can act on "this change contains no
            // async task boundary", and it used to render as a WARNING sitting
            // among the real failures on every pull request that touches none.
            GateStatus::NotApplicable { subject, .. } => {
                declared_absent.push(format!("- **{}** — {subject}", gate_name(gate_id)));
                None
            }
            GateStatus::Passed | GateStatus::AutoUpdated => None,
        };
        if let Some(l) = line {
            findings.push(l);
            if understates_itself(gate_id) {
                understated_findings.push(gate_name(gate_id));
            }
        }
    }

    // One line, folded, naming the gates and why they are absent. Deterministic
    // ordering, per rule 5.
    let absence_note = |plural: bool| -> String {
        if declared_absent.is_empty() {
            return String::new();
        }
        format!(
            "\n<details><summary>{} gate{} absent by declaration \u{2014} the capability is not \
             provisioned here, or this change carries no subject for them</summary>\n\n{}\n\
             </details>\n",
            declared_absent.len(),
            if plural { "s" } else { "" },
            declared_absent.join("\n")
        )
    };

    let mut s = String::new();
    if report.is_admissible() {
        // The tally is stated in full rather than collapsed to "passed".
        // `gate_counts` once scored `is_acceptable()`, which is true for both
        // `Warning` and `NotMeasured`, so a corpus that measured nothing
        // rendered here as every gate passing. A reader cannot discount what
        // the number never showed them.
        let mut headline = format!("✅ Certified — {passed}/{total} gates passed");
        let mut qualifiers = Vec::new();
        if counts.warned > 0 {
            qualifiers.push(format!("{} warned", counts.warned));
        }
        if counts.unmeasured > 0 {
            // Said as what it is. "unmeasured" alone invites a reader to
            // discount a real measurement failure alongside a capability this
            // deployment simply does not have.
            let blocking_absences = counts.unmeasured - declared_absent.len();
            if blocking_absences > 0 {
                qualifiers.push(format!("{blocking_absences} unmeasured"));
            }
            if !declared_absent.is_empty() {
                qualifiers.push(format!("{} absent by declaration", declared_absent.len()));
            }
        }
        if !qualifiers.is_empty() {
            headline.push_str(&format!(" ({})", qualifiers.join(", ")));
        }
        headline.push_str(".\n");
        s.push_str(&headline);
        // A finding on a certified pull request is still a finding.
        //
        // `findings` was built above on both branches and emitted on one. A
        // `Warning` is `is_acceptable()`, so a report whose only findings are
        // warnings certifies -- the warning could not reach the blocked branch
        // by itself, and was discarded on the only branch it could reach. All
        // the whole corpus was exposed: the two capped scanner gates, and
        // `trace_context_guard`, which chose `Warning` over `Passed` in so many
        // words *to avoid* rendering as a bare tick.
        //
        // Under a heading that says what the block is: an unlabelled finding
        // beneath a green verdict reads as a defect that blocked nothing for no
        // stated reason, and a reader learns to skip it. Nothing is emitted when
        // there is nothing to say, so a clean scorecard is unchanged.
        if !findings.is_empty() {
            s.push_str(&format!(
                "\n⚠️ {} advisory finding(s) — acceptable, not blocking this merge:\n\n",
                findings.len()
            ));
            s.push_str(&findings.join("\n"));
            s.push('\n');
            s.push_str(&understatement_note(&understated_findings));
        }
        // A passing gate produces no finding line, so `understatement_note`
        // says nothing about it. Without the line below the disclosure would
        // reach only the failure path -- and the green path is the one moment a
        // reader decides whether to trust the score. What is behind the number
        // is load-bearing precisely when the number is good.
        let understated = low_fidelity_passing_gates(report);
        if !understated.is_empty() {
            // The unaudited count rides on this existing line rather than
            // taking one of its own.
            //
            // `report.rs` withholds a verdict from a gate with no registry
            // entry, and defends that exemption with "is not silent:
            // `fidelity::gap_report().unaudited` publishes its size". It
            // published nothing -- `gap_report` had no caller outside
            // `#[cfg(test)]`, so the exemption was silent and the sentence
            // justifying it rested on a mechanism that did not run.
            //
            // It is not in the verdict line: how many gates nobody has audited
            // is a fact about the registry, not an outcome of this run, and
            // folding it into "N/M gates passed" conflates the two. It is not a
            // paragraph of its own either: the certified path is the common one
            // and `scorecard_wiring_test` caps it at three content lines so
            // nothing buries the verdict.
            let unaudited = crate::fidelity::gap_report(TOTAL_GATES).unaudited;
            let unaudited_note = if unaudited > 0 {
                format!(
                    " A further {unaudited} of {TOTAL_GATES} have no registry \
                     entry at all, so nothing here claims anything about them."
                )
            } else {
                String::new()
            };
            s.push_str(&format!(
                "\n⚠️ {} of the passing gates do not fully measure what their \
                 names imply: {}. See `src/fidelity/registry.rs` for what each \
                 one actually checks.{}\n",
                understated.len(),
                understated.join(", "),
                unaudited_note
            ));
        }
    } else {
        // The headline is what needs action, not the size of the corpus. It
        // used to read "38 finding(s) across 72 gates; 34 gate(s) produced no
        // measurement", of which four were things a reader could do something
        // about.
        s.push_str(&format!(
            "❌ Blocked — {} finding(s) need action; {passed}/{total} gates passed.\n\n",
            findings.len()
        ));
        s.push_str(&findings.join("\n"));
        s.push('\n');
        s.push_str(&understatement_note(&understated_findings));

        // The prevention ledger, where a reader is already acting.
        //
        // It records each defect CLASS, the layer each remedy sits at and
        // whether that remedy is mechanical or semantic -- and it was
        // unreachable from production, running only from a pre-push test. A
        // ledger nobody reads does not change what anyone does, which is the
        // same defect as a gate nothing calls.
        //
        // On the BLOCKED path only. Three guards refused this line on the
        // certified scorecard and were right to: a certified verdict is one
        // counted line by contract, and this measures the state of the
        // repository rather than anything about the change. Published rather
        // than gated, for the same reason -- a finding a change cannot act on
        // must not withhold it.
        s.push('\n');
        s.push_str(&crate::postmortem::prevention_debt_line());
        s.push('\n');

        // What the passing half of this score is worth.
        //
        // `gate_proof` was complete, ratcheted and called by nothing: it knew
        // which gates have been seeded with their own defect and which have only
        // ever been green, and no pull request was ever told. A gate that has
        // never been shown to fire still prints a tick indistinguishable from
        // a gate that has, which is precisely how four checks written in one
        // session stayed green for the defect they existed to catch.
        //
        // Published, not gated, and on the blocked path only -- the same two
        // constraints `prevention_debt_line` above is under, for the same two
        // reasons. An author cannot act on a gate their change never touched,
        // and a certified verdict is one counted line by contract.
        let ids = passing_gate_ids(report);
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        if let Some(line) = super::proof_line::qualifier(&refs) {
            s.push_str(&line);
            s.push('\n');
        }
    }

    s.push_str(&absence_note(declared_absent.len() != 1));

    let action = if report.is_admissible() {
        AnvilAction::Certified
    } else {
        AnvilAction::Blocked
    };
    // The report already carries the sha it judged; publishing without it
    // would leave the verdict unanchored across a force-push.
    // The subject carries the sha the run was performed against. A report with
    // no subject was not produced by a certification run over a commit, so it
    // is NotRevisionScoped rather than anchored to a sha invented here.
    let judged = match report.subject() {
        Some(s) => crate::publish::Judged::Rev(s.head_sha.clone()),
        None => crate::publish::Judged::NotRevisionScoped,
    };
    body(action, &s, judged).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gate_names_are_stable_and_undecorated() {
        assert_eq!(gate_name("doc_parity_status"), "doc-parity");
        assert_eq!(gate_name("slo_status"), "slo");
    }

    #[test]
    fn remediation_is_absent_rather_than_invented() {
        assert!(remediation_for("coverage_status").is_some());
        assert!(remediation_for("gate_with_no_known_fix_status").is_none());
    }

    #[test]
    fn a_finding_states_what_and_how() {
        let l = finding_line(
            "coverage_status",
            "failed",
            "62.0% is below the required 85%",
        );
        assert!(l.starts_with("- **coverage** — failed: 62.0%"));
        assert!(l.contains("fix: add tests covering the lines this PR adds"));
    }

    #[test]
    fn low_fidelity_gates_are_flagged_so_a_verdict_is_not_overtrusted() {
        // Both halves: a gate the registry records below `Measured` reaches
        // the disclosure, and one recorded as `Measured` does not -- a note
        // naming every gate discloses nothing.
        assert!(understates_itself("coverage_status"));
        assert!(!understates_itself("shape_status"));
        let note = understatement_note(&[gate_name("coverage_status")]);
        assert!(note.contains("do not fully measure"), "{note}");
        assert!(note.contains("coverage"), "{note}");
        assert!(understatement_note(&[]).is_empty());
    }

    #[test]
    fn a_finding_with_no_known_fix_renders_no_hint() {
        let l = finding_line("some_unknown_status", "failed", "x");
        assert!(!l.contains("fix:"), "must not invent remediation: {l}");
    }
}
