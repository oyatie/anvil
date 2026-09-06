//! What one gate said, and what that entitles a report to claim.
//!
//! Its own file because `report.rs` is the largest in this crate and far past
//! ADR-0719 D-35's budget, and because these are two things: `GateStatus` is
//! one gate's answer, `PreMergeCertificationReport` is the whole corpus's.
//!
//! The distinctions here are the ones a boolean cannot carry -- `NotMeasured`
//! from `NotApplicable`, `Errored` from `Failed` -- and they are what makes
//! invariant I1 expressible at all.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateStatus {
    Passed,
    AutoUpdated,
    Warning(String),
    Failed(String),
    /// The gate was configured to run and had a data source, but could not
    /// produce a measurement: the tool was missing, the subprocess failed to
    /// spawn, the call timed out, or the response could not be parsed.
    ///
    /// This is NOT acceptable. Invariant I1: absent evidence is never a pass.
    Errored(String),
    /// The gate has no data source configured, so it makes no claim in either
    /// direction. Acceptable on its own — reporting a failure here would be a
    /// fabricated accusation, the symmetric violation of I1 — but it is
    /// recorded in `PreMergeCertificationReport::unmeasured_gates` and blocks
    /// merge-queue admission separately from `is_certified_ready`.
    NotMeasured {
        gate_id: String,
        reason: String,
    },
    /// The gate ran, searched a named subject set, and found it empty.
    ///
    /// Not a pass and not a defect: the correct outcome for a change that
    /// carries no subject for this gate. Acceptable, and it does not withhold
    /// merge-queue admission.
    ///
    /// Separate from `NotMeasured` because one gate can produce both. The trace
    /// guard reports this when a diff crosses no async boundary, and
    /// `NotMeasured` when it crosses boundaries it could not resolve -- the
    /// first is complete evidence that happens to be empty, the second is
    /// missing evidence. A per-gate table cannot tell those apart, so the gate
    /// says which it is.
    NotApplicable {
        gate_id: String,
        subject: String,
    },
}

impl GateStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            GateStatus::Passed => "✅ PASSED",
            GateStatus::AutoUpdated => "✨ AUTO-SYNCED",
            GateStatus::Warning(_) => "⚠️ WARNING",
            GateStatus::Failed(_) => "❌ FAILED",
            GateStatus::Errored(_) => "🛑 ERRORED",
            GateStatus::NotMeasured { .. } => "➖ NOT MEASURED",
            GateStatus::NotApplicable { .. } => "➖ NOT APPLICABLE",
        }
    }

    /// Whether this status permits certification.
    ///
    /// `Errored` is deliberately false: a gate that could not measure must not
    /// pass. `NotMeasured` is deliberately true, because an unconfigured gate
    /// has not found a defect — it is gated instead via `unmeasured_gates`.
    pub fn is_acceptable(&self) -> bool {
        match self {
            GateStatus::Passed | GateStatus::AutoUpdated => true,
            GateStatus::Warning(_) => true,
            GateStatus::Failed(_) => false,
            GateStatus::Errored(_) => false,
            GateStatus::NotMeasured { .. } => true,
            // The gate ran and its subject set was empty. That is complete
            // evidence which happens to be empty, not absent evidence.
            GateStatus::NotApplicable { .. } => true,
        }
    }

    /// Whether this gate actually produced a measurement.
    pub fn is_measured(&self) -> bool {
        !matches!(self, GateStatus::NotMeasured { .. })
    }

    /// The gate id, when this status is `NotMeasured`.
    pub fn unmeasured_gate_id(&self) -> Option<&str> {
        match self {
            GateStatus::NotMeasured { gate_id, .. } => Some(gate_id.as_str()),
            _ => None,
        }
    }
}
