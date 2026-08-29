//! The certification gate: judge a pull request's head against the shape
//! baseline frozen at its merge-base, and say exactly what was measured.
//!
//! Returns a shape-side outcome; the certification evaluator maps it to a
//! `GateStatus`. That keeps this unit free of the certification vocabulary
//! and keeps the mapping — what blocks, what warns, what is not measured —
//! next to the other gates where it is reviewed.

use super::baseline::{Judgement, judge};
use crate::ratchet::facade::Mode;
use crate::shape::ports::{ShapeDistance, SpecSource};
use std::collections::BTreeMap;
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

fn measurement_of(j: &Judgement) -> ShapeMeasurement {
    let (report, verdict) = match j {
        Judgement::Bootstrap { report, .. } => (report, None),
        Judgement::Judged {
            report, verdict, ..
        } => (report, Some(verdict)),
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
