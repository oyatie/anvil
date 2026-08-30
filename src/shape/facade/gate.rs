//! The certification gate: judge a pull request's head against the shape
//! baseline frozen at its merge-base, and say exactly what was measured.
//!
//! Returns a shape-side outcome; the certification evaluator maps it to a
//! `GateStatus`. That keeps this unit free of the certification vocabulary
//! and keeps the mapping — what blocks, what warns, what is not measured —
//! next to the other gates where it is reviewed.

use super::baseline::{Judgement, judge};
use crate::ratchet::facade::Mode;
use crate::shape::ports::{ShapeDistance, ShapeReport, SpecSource};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// What the gate measured, for telemetry and the fleet view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShapeMeasurement {
    pub repo: String,
    pub rev: String,
    pub spec_source: String,
    pub distance: ShapeDistance,
    pub per_rule: BTreeMap<String, usize>,
    pub blocking_regressions: usize,
    pub advisory_regressions: usize,
    pub fixed: usize,
    /// Rules the tenant declared in a blocking mode that the measurement could
    /// not evaluate, each with the reason, as `rule: why`.
    ///
    /// A rule that was not measured contributes no keys, so it can regress
    /// nothing and the ratchet has nothing to refuse. Publishing that as a
    /// pass would let absent evidence read as conformance (I1), so the gate
    /// withholds instead — and it withholds only for rules the tenant asked to
    /// be blocked on, because an advisory rule that could not run refuses
    /// nothing either way.
    pub blocking_unmeasured: Vec<String>,
}

impl ShapeMeasurement {
    /// Why this measurement is not one, when a blocking rule did not run.
    pub fn unmeasured_reason(&self) -> Option<String> {
        (!self.blocking_unmeasured.is_empty()).then(|| self.blocking_unmeasured.join("; "))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShapeGateOutcome {
    /// The tree carries no `.anvil/shape.json` at the head commit.
    NoSpec { reason: String },
    /// A spec exists at the head but does not parse or is invalid.
    SpecUnreadable { reason: String },
    /// Git could not be read.
    Errored { reason: String },
    /// Measured, but no baseline exists at the merge-base: this change
    /// introduces one, so regressions cannot be judged yet.
    Bootstrap { measurement: ShapeMeasurement },
    /// Measured and judged against the frozen baseline.
    Judged {
        measurement: ShapeMeasurement,
        /// Keys new since the baseline under blocking rules: `rule: key`.
        blocking: Vec<String>,
    },
}

/// The blocking rules `report` could not evaluate, as `rule: why`.
///
/// Joined against the rule set the head spec declares, never against the
/// verdict: a rule that was not measured produced no findings and may have no
/// baseline entry either, so the verdict is exactly where it would be missing.
pub fn blocking_unmeasured(report: &ShapeReport, blocking_rules: &BTreeSet<String>) -> Vec<String> {
    report
        .not_measured
        .iter()
        .filter(|(rule, _)| blocking_rules.contains(&rule.0))
        .map(|(rule, why)| format!("{rule}: {why}"))
        .collect()
}

fn measurement_of(j: &Judgement) -> ShapeMeasurement {
    let empty = BTreeSet::new();
    let (report, verdict, blocking_rules) = match j {
        Judgement::Bootstrap { report, .. } => (report, None, &empty),
        Judgement::Judged {
            report,
            verdict,
            blocking_rules,
            ..
        } => (report, Some(verdict), blocking_rules),
    };
    let mut per_rule: BTreeMap<String, usize> = BTreeMap::new();
    for f in &report.findings {
        *per_rule.entry(f.rule.0.clone()).or_default() += 1;
    }
    let (blocking, advisory, fixed) = verdict
        .map(|v| {
            v.per_rule.values().fold((0, 0, 0), |(b, a, f), r| {
                let n = r.regressions.len();
                (
                    b + if r.mode == Mode::BlockOnNew { n } else { 0 },
                    a + if r.mode == Mode::Advisory { n } else { 0 },
                    f + r.fixed.len(),
                )
            })
        })
        .unwrap_or((0, 0, 0));
    let blocking_unmeasured = blocking_unmeasured(report, blocking_rules);
    ShapeMeasurement {
        repo: report.repo.clone(),
        rev: report.rev.clone(),
        spec_source: match &report.spec_source {
            SpecSource::Adopted => "adopted".into(),
            SpecSource::Proposed(p) => format!("proposed:{p}"),
            SpecSource::CandidateBootstrap => "candidate".into(),
        },
        distance: report.distance(),
        per_rule,
        blocking_regressions: blocking,
        advisory_regressions: advisory,
        fixed,
        blocking_unmeasured,
    }
}

/// `base_branch` is the branch the PR targets (from the PR context, never
/// from the change); `head` is the PR head sha.
pub async fn judge_pr(
    repo_dir: &Path,
    base_branch: &str,
    head: &str,
    repo_label: &str,
) -> ShapeGateOutcome {
    let base_ref = format!("origin/{}", base_branch.trim_start_matches("origin/"));
    match judge(repo_dir, &base_ref, head, None).await {
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("pass --spec-override") {
                ShapeGateOutcome::NoSpec { reason: msg }
            } else if msg.contains("shape spec does not parse")
                || msg.contains("shape spec is invalid")
                || msg.contains("spec is not UTF-8")
            {
                ShapeGateOutcome::SpecUnreadable { reason: msg }
            } else {
                ShapeGateOutcome::Errored { reason: msg }
            }
        }
        Ok(j) => {
            let mut m = measurement_of(&j);
            m.repo = repo_label.to_string();
            match &j {
                Judgement::Bootstrap { .. } => ShapeGateOutcome::Bootstrap { measurement: m },
                Judgement::Judged { verdict, .. } => {
                    let mut blocking = Vec::new();
                    for (rule, v) in &verdict.per_rule {
                        if v.mode == Mode::BlockOnNew {
                            blocking.extend(v.regressions.iter().map(|k| format!("{rule}: {k}")));
                        }
                    }
                    for (rule, key) in &verdict.inert_signoff {
                        blocking.push(format!("{rule}: inert signoff for {key}"));
                    }
                    ShapeGateOutcome::Judged {
                        measurement: m,
                        blocking,
                    }
                }
            }
        }
    }
}

impl ShapeGateOutcome {
    pub fn measurement(&self) -> Option<&ShapeMeasurement> {
        match self {
            ShapeGateOutcome::Bootstrap { measurement }
            | ShapeGateOutcome::Judged { measurement, .. } => Some(measurement),
            ShapeGateOutcome::NoSpec { .. }
            | ShapeGateOutcome::SpecUnreadable { .. }
            | ShapeGateOutcome::Errored { .. } => None,
        }
    }

    /// One line: `distance N (units M/K conformant, B new on blocking rules, A advisory)`.
    pub fn summary(&self) -> String {
        match self {
            ShapeGateOutcome::NoSpec { reason }
            | ShapeGateOutcome::SpecUnreadable { reason }
            | ShapeGateOutcome::Errored { reason } => reason.clone(),
            ShapeGateOutcome::Bootstrap { measurement: m } => format!(
                "distance {} (units {}/{} conformant); no baseline at merge-base — this change bootstraps it",
                m.distance.findings_total, m.distance.units_conformant, m.distance.units_total
            ),
            ShapeGateOutcome::Judged {
                measurement: m,
                blocking,
            } => format!(
                "distance {} (units {}/{} conformant, {} fixed, {} new on advisory rules, {} new on blocking rules)",
                m.distance.findings_total,
                m.distance.units_conformant,
                m.distance.units_total,
                m.fixed,
                m.advisory_regressions,
                blocking.len()
            ),
        }
    }
}
