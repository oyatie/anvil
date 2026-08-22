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
use crate::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport};
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
fn finding_line(gate_id: &str, kind: &str, detail: &str) -> String {
    let mut s = format!("- **{}** — {}: {}", gate_name(gate_id), kind, detail.trim());
    if let Some(fix) = remediation_for(gate_id) {
        s.push_str(&format!("\n  - fix: {}", fix));
    }
    if let Some(f) = fidelity_for(gate_id)
        && f < Fidelity::Measured
    {
        s.push_str(&format!(
            "\n  - note: this gate is {} fidelity and does not fully measure what its name implies",
            f.label().to_lowercase()
        ));
    }
    s
}

/// The passing gates the fidelity registry records as below `Measured`.
///
/// A gate can pass on a keyword scan; the registry is where that is written
/// down. Naming them next to the score is what stops "72/72" from being read
/// as 72 measurements.
fn low_fidelity_passing_gates(report: &PreMergeCertificationReport) -> Vec<String> {
    report
        .named_statuses()
        .into_iter()
        .filter(|(_, status)| matches!(status, GateStatus::Passed | GateStatus::AutoUpdated))
        .filter(|(gate_id, _)| fidelity_for(gate_id).is_some_and(|f| f < Fidelity::Measured))
        .map(|(gate_id, _)| gate_name(gate_id))
        .collect()
}

/// Renders the scorecard body, signature included.
pub fn render(report: &PreMergeCertificationReport) -> String {
    let (passed, failed) = report.gate_counts();
    let total = passed + failed;

    let mut findings: Vec<String> = Vec::new();
    for (gate_id, status) in report.named_statuses() {
        let line = match status {
            GateStatus::Failed(r) => Some(finding_line(gate_id, "failed", r)),
            GateStatus::Errored(r) => Some(finding_line(gate_id, "errored", r)),
            GateStatus::NotMeasured { reason, .. } => {
                Some(finding_line(gate_id, "not measured", reason))
            }
            GateStatus::Warning(r) => Some(finding_line(gate_id, "warning", r)),
            GateStatus::Passed | GateStatus::AutoUpdated => None,
        };
        if let Some(l) = line {
            findings.push(l);
        }
    }

    let mut s = String::new();
    if report.is_admissible() {
        s.push_str(&format!(
            "✅ Certified — {}/{} gates passed.\n",
            passed, total
        ));
        // A passing gate produces no finding line, so it never carried the
        // fidelity note that `finding_line` attaches. That put the disclosure
        // only on the failure path -- and the green path is the one moment a
        // reader decides whether to trust the score. What is behind the number
        // is load-bearing precisely when the number is good.
        let understated = low_fidelity_passing_gates(report);
        if !understated.is_empty() {
            s.push_str(&format!(
                "\n⚠️ {} of the passing gates do not fully measure what their \
                 names imply: {}. See `src/fidelity/registry.rs` for what each \
                 one actually checks.\n",
                understated.len(),
                understated.join(", ")
            ));
        }
    } else {
        let unmeasured = report.unmeasured_gates.len();
        s.push_str(&format!(
            "❌ Blocked — {} finding(s) across {} gates{}.\n\n",
            findings.len(),
            total,
            if unmeasured > 0 {
                format!("; {} gate(s) produced no measurement", unmeasured)
            } else {
                String::new()
            }
        ));
        s.push_str(&findings.join("\n"));
        s.push('\n');
    }

    let action = if report.is_admissible() {
        AnvilAction::Certified
    } else {
        AnvilAction::Blocked
    };
    body(action, &s)
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
        let l = finding_line("coverage_status", "failed", "x");
        assert!(l.contains("aspirational fidelity"));
    }

    #[test]
    fn a_finding_with_no_known_fix_renders_no_hint() {
        let l = finding_line("some_unknown_status", "failed", "x");
        assert!(!l.contains("fix:"), "must not invent remediation: {l}");
    }
}
