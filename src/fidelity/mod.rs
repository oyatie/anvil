//! Gate fidelity: the declared distance between aspiration and reality.
//!
//! # Why this exists
//!
//! Anvil's gates are named for hyperscaler capabilities -- "Kani formal
//! verification", "differential coverage", "OpenSLO burn rate", "Sigstore
//! provenance" -- while several were implemented as regexes over raw diffs or as
//! hardcoded constants. Four were unfailable by arithmetic: a fabricated value
//! compared against a bound it could never cross.
//!
//! The root cause is structural, not careless. Aspiration and reality lived in
//! the same place -- the gate's name and its prose -- so drift between them was
//! invisible and required no lie to occur. Nobody wrote "this is formal
//! verification" untruthfully; the name simply outlived the implementation.
//!
//! # The approach
//!
//! Write the aspiration first and explicitly, as the capability a hyperscaler
//! would actually operate. Then declare, separately and machine-readably, how
//! much of it is real. The gap is the difference, and it is *computed* rather
//! than editorial.
//!
//! Three properties follow:
//!
//! 1. **A gate cannot silently overclaim.** Fidelity is an enum, not prose.
//! 2. **The gap is publishable.** It can be rendered on the scorecard and the
//!    dashboard, so incompleteness is visible rather than implied.
//! 3. **Drift is detectable.** `audit_against_reality` re-derives what is
//!    observably true and flags any gate whose declared fidelity exceeds it.
//!
//! # Relationship to `GateStatus`
//!
//! `GateStatus` answers "what did this gate find on this PR?". `Fidelity`
//! answers "how much should you trust that answer at all?". A gate at
//! `Aspirational` fidelity must report `GateStatus::NotMeasured`, which blocks
//! merge admission via `unmeasured_gates` while making no false accusation.

pub mod registry;

use serde::{Deserialize, Serialize};

/// How faithfully a gate implements the capability its name claims.
///
/// Ordered: each level is strictly stronger than the one before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Fidelity {
    /// Named only. No implementation of the claimed capability exists.
    /// Must report `GateStatus::NotMeasured`.
    Aspirational,
    /// A proxy signal -- a regex, a line count, a filename match. It may
    /// correlate with the real property, but it does not measure it. A
    /// heuristic is legitimate; presenting it as the real capability is not.
    Heuristic,
    /// A real tool runs, but coverage is incomplete, or the result is partial,
    /// or some failure mode is still unhandled.
    Partial,
    /// Actually measures the property it claims, AND a seeded-defect fixture
    /// demonstrates it can fail. Both halves are required: an implementation
    /// that cannot be shown to fail is indistinguishable from a constant.
    Measured,
}

impl Fidelity {
    pub const fn label(self) -> &'static str {
        match self {
            Fidelity::Aspirational => "ASPIRATIONAL",
            Fidelity::Heuristic => "HEURISTIC",
            Fidelity::Partial => "PARTIAL",
            Fidelity::Measured => "MEASURED",
        }
    }

    /// Whether a gate at this fidelity is entitled to report a pass.
    ///
    /// An aspirational gate has nothing to say, so it must not say "passed".
    pub const fn may_report_pass(self) -> bool {
        !matches!(self, Fidelity::Aspirational)
    }
}

/// One gate's declared aspiration, its measured reality, and the gap between.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateFidelity {
    /// Matches the field name on `PreMergeCertificationReport`.
    pub gate_id: &'static str,
    /// The capability as a hyperscaler would actually operate it. Written
    /// first, deliberately, whether or not it is implemented yet.
    pub aspiration: &'static str,
    /// A citable basis for the aspiration: a tool, a paper, a named practice.
    /// Prevents the aspiration itself from drifting into invention.
    pub reference: &'static str,
    pub fidelity: Fidelity,
    /// The honest delta: what this gate does NOT do. Required at every fidelity
    /// below `Measured`, and asserted as non-empty by a test.
    pub gap: &'static str,
    /// What must exist before the gap can close. `None` means nothing external
    /// is blocking it -- the work is simply not done.
    pub blocked_on: Option<&'static str>,
}

/// A discrepancy between what a gate claims and what is observably true.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FidelityDrift {
    pub gate_id: String,
    pub declared: Fidelity,
    pub observed: Fidelity,
    pub evidence: String,
}

/// Aggregate view of how far the matrix is from its own aspiration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapReport {
    pub audited: usize,
    pub aspirational: usize,
    pub heuristic: usize,
    pub partial: usize,
    pub measured: usize,
    /// Gates on `PreMergeCertificationReport` with no entry in this registry.
    /// Non-zero means the audit itself is incomplete, which is a gap in the
    /// gap report and is reported rather than hidden.
    pub unaudited: usize,
    pub drift: Vec<FidelityDrift>,
}

impl GapReport {
    /// Fraction of audited gates that genuinely measure what they claim.
    pub fn honesty_ratio(&self) -> f64 {
        if self.audited == 0 {
            return 0.0;
        }
        self.measured as f64 / self.audited as f64
    }

    pub fn summary(&self) -> String {
        format!(
            "Gate fidelity: {} measured, {} partial, {} heuristic, {} aspirational \
             ({} audited, {} not yet audited, {} drifting)",
            self.measured,
            self.partial,
            self.heuristic,
            self.aspirational,
            self.audited,
            self.unaudited,
            self.drift.len()
        )
    }
}

/// Builds the gap report from the registry.
///
/// `total_gates` should be `PreMergeCertificationReport::all_statuses().len()`,
/// so that gates never audited are surfaced rather than silently excluded.
pub fn gap_report(total_gates: usize) -> GapReport {
    let entries = registry::AUDITED_GATES;
    let count = |f: Fidelity| entries.iter().filter(|e| e.fidelity == f).count();
    GapReport {
        audited: entries.len(),
        aspirational: count(Fidelity::Aspirational),
        heuristic: count(Fidelity::Heuristic),
        partial: count(Fidelity::Partial),
        measured: count(Fidelity::Measured),
        unaudited: registry::unaudited_count(total_gates),
        drift: Vec::new(),
    }
}

/// Observable evidence a gate can be checked against.
///
/// Deliberately mechanical: whether the tool a gate depends on is present, and
/// whether the gate has a seeded-defect fixture. These are facts about the
/// environment and the test suite, not opinions, so a declared fidelity can be
/// contradicted by them.
#[derive(Debug, Clone, Copy)]
pub struct Evidence {
    /// The external tool the aspiration requires is installed and runnable.
    pub tool_available: bool,
    /// The gate actually invokes that tool.
    pub tool_invoked: bool,
    /// A seeded-defect fixture demonstrates this gate can fail (I9).
    pub failing_fixture_exists: bool,
}

/// Re-derives the highest fidelity the evidence supports.
///
/// This is the "evaluate reality against aspiration" half of the loop. It is
/// intentionally conservative: `Measured` requires BOTH that the tool runs and
/// that a fixture proves the gate can fail, because an implementation which
/// cannot be shown to fail is indistinguishable from a constant -- which is how
/// the coverage gate reached `.max(85.0)` in the first place.
pub fn observed_fidelity(e: Evidence) -> Fidelity {
    match (e.tool_invoked, e.tool_available, e.failing_fixture_exists) {
        (true, true, true) => Fidelity::Measured,
        (true, true, false) => Fidelity::Partial,
        // Invokes a tool that is not installed: the fallback path is what
        // actually runs, so the gate is no better than its heuristic.
        (true, false, _) => Fidelity::Heuristic,
        (false, _, _) => Fidelity::Aspirational,
    }
}

/// Flags every gate whose declared fidelity exceeds what the evidence supports.
///
/// Overclaiming is a defect; underclaiming is not. A gate may legitimately
/// declare less than it delivers, but never more.
pub fn audit_against_reality<F>(mut evidence_for: F) -> Vec<FidelityDrift>
where
    F: FnMut(&str) -> Option<Evidence>,
{
    let mut drift = Vec::new();
    for entry in registry::AUDITED_GATES {
        let Some(e) = evidence_for(entry.gate_id) else {
            continue;
        };
        let observed = observed_fidelity(e);
        if entry.fidelity > observed {
            drift.push(FidelityDrift {
                gate_id: entry.gate_id.to_string(),
                declared: entry.fidelity,
                observed,
                evidence: format!(
                    "tool_available={} tool_invoked={} failing_fixture={}",
                    e.tool_available, e.tool_invoked, e.failing_fixture_exists
                ),
            });
        }
    }
    drift
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_below_measured_states_its_gap() {
        for e in registry::AUDITED_GATES {
            if e.fidelity != Fidelity::Measured {
                assert!(
                    !e.gap.trim().is_empty(),
                    "{} must state what it does not do",
                    e.gate_id
                );
            }
            assert!(
                !e.aspiration.trim().is_empty(),
                "{} needs an aspiration",
                e.gate_id
            );
            assert!(
                !e.reference.trim().is_empty(),
                "{} needs a citable reference",
                e.gate_id
            );
        }
    }

    #[test]
    fn gate_ids_are_unique() {
        let mut ids: Vec<_> = registry::AUDITED_GATES.iter().map(|e| e.gate_id).collect();
        let before = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate gate_id in the registry");
    }

    #[test]
    fn aspirational_gates_may_not_report_a_pass() {
        assert!(!Fidelity::Aspirational.may_report_pass());
        for f in [Fidelity::Heuristic, Fidelity::Partial, Fidelity::Measured] {
            assert!(f.may_report_pass());
        }
    }

    #[test]
    fn measured_requires_both_a_running_tool_and_a_failing_fixture() {
        // The whole point: an implementation that cannot be shown to fail is
        // indistinguishable from a hardcoded constant.
        assert_eq!(
            observed_fidelity(Evidence {
                tool_available: true,
                tool_invoked: true,
                failing_fixture_exists: false
            }),
            Fidelity::Partial
        );
        assert_eq!(
            observed_fidelity(Evidence {
                tool_available: true,
                tool_invoked: true,
                failing_fixture_exists: true
            }),
            Fidelity::Measured
        );
    }

    #[test]
    fn invoking_an_absent_tool_is_only_heuristic() {
        // The shape a gate takes when it names a tool it never reaches: the
        // fallback is what really runs, so the fallback is what it may claim.
        assert_eq!(
            observed_fidelity(Evidence {
                tool_available: false,
                tool_invoked: true,
                failing_fixture_exists: true
            }),
            Fidelity::Heuristic
        );
    }

    #[test]
    fn drift_is_reported_when_a_gate_claims_more_than_the_evidence_supports() {
        // Claim Partial for doc_parity while nothing is actually invoked.
        let drift = audit_against_reality(|id| {
            (id == "doc_parity_status").then_some(Evidence {
                tool_available: false,
                tool_invoked: false,
                failing_fixture_exists: false,
            })
        });
        assert_eq!(drift.len(), 1);
        assert_eq!(drift[0].gate_id, "doc_parity_status");
        assert_eq!(drift[0].observed, Fidelity::Aspirational);
    }

    #[test]
    fn underclaiming_is_not_drift() {
        // Declared Heuristic, evidence supports Measured -> no complaint.
        let drift = audit_against_reality(|id| {
            (id == "mutation_status").then_some(Evidence {
                tool_available: true,
                tool_invoked: true,
                failing_fixture_exists: true,
            })
        });
        assert!(drift.is_empty());
    }

    #[test]
    fn the_report_admits_how_much_is_unaudited() {
        let r = gap_report(68);
        assert_eq!(r.audited, registry::AUDITED_GATES.len());
        assert_eq!(r.audited + r.unaudited, 68);
        assert!(
            r.unaudited > 0,
            "the audit is not yet complete and must say so"
        );
        // Nothing has been made real yet; the honest ratio is zero.
        assert_eq!(r.measured, 0);
        assert_eq!(r.honesty_ratio(), 0.0);
    }
}
