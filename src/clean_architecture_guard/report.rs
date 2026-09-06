//! The shape of a clean-architecture verdict.
//!
//! Split out of one 928-line file under ADR-0719 D-35: 300 physical lines is
//! the born-blocking maximum for a hand-written file, and a split is more
//! `snake_case.rs` modules inside the same crate -- not a new crate, and not
//! a face taxonomy, which D-8 forbids a unit from inventing.

use crate::pre_merge_guard::GateStatus;
use serde::{Deserialize, Serialize};

pub(super) fn build_report(
    scope: String,
    violations: Vec<ArchViolation>,
    files_inspected: usize,
    files_classified: usize,
    rust_files_inspected: usize,
    face_subjects: usize,
    unknown_faces: usize,
) -> CleanArchitectureReport {
    // Two rules with two different subjects. The layer-direction rules need
    // a file that sits in a layer; the facade seal needs only Rust source.
    // Keying the verdict on the first alone reported a real, FOUND bypass as
    // "NOT MEASURED ... no inward-dependency claim can be made" whenever the
    // offending file happened to sit in no layer -- which is the common case,
    // and the exact case the seal exists for. An absence must never outrank a
    // finding.
    let measurement = if unknown_faces > 0 {
        ArchMeasurement::Unavailable {
            reason: format!(
                "target ownership unresolved for {unknown_faces} face reference(s); {files_classified} layered file(s) and {face_subjects} resolved face reference(s) checked"
            ),
            files_inspected,
        }
    } else if files_classified == 0 && face_subjects == 0 {
        ArchMeasurement::NotMeasured {
            reason: format!(
                "nothing to measure in {files_inspected} file(s) examined: 0 belong to a \
                 supported, checked layer, and no resolved path names any unit's core/ports/adapters, so \
                 neither the layer-direction rules nor the facade seal had a subject"
            ),
            files_inspected,
        }
    } else {
        ArchMeasurement::Measured {
            files_inspected,
            files_classified,
        }
    };

    // An unmeasured run is not a clean run.
    let is_clean = violations.is_empty() && measurement.is_measured();

    let summary = match &measurement {
        ArchMeasurement::Unavailable { reason, .. } => format!(
            "Clean Architecture UNAVAILABLE for {scope}: {reason}. {} definite violation(s): {}",
            violations.len(),
            violations
                .iter()
                .map(|v| format!("{}: {}", v.file_path, v.description))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        ArchMeasurement::NotMeasured { reason, .. } => format!(
            "Clean Architecture NOT MEASURED completely for {scope}: {reason}. {} definite violation(s): {}",
            violations.len(),
            violations
                .iter()
                .map(|v| format!("{}: {}", v.file_path, v.description))
                .collect::<Vec<_>>()
                .join("; ")
        ),
        ArchMeasurement::Measured {
            files_inspected,
            files_classified,
        } => {
            if violations.is_empty() {
                // Say exactly what was measured. "Verified ... 100% intact"
                // over 8 layered files of 256 is a claim about the 248 the
                // classifier never saw (I2).
                format!(
                    "Clean Architecture: 0 layer-boundary violations across \
                     {files_classified} layered file(s) of {files_inspected} examined in \
                     {scope}; {} file(s) were not eligible added layer-source subjects and were not \
                     measured for layer direction. The facade seal examined \
                     {face_subjects} face reference(s) across {rust_files_inspected} Rust \
                     file(s).",
                    files_inspected.saturating_sub(*files_classified)
                )
            } else {
                // The denominator belongs on this branch too. A findings
                // list alone says nothing about the files the classifier
                // never saw, and "18 violations" reads as a complete
                // account of the tree when it is an account of 55 files.
                format!(
                    "Clean Architecture layer boundary violations ({} items) across \
                     {files_classified} layered file(s) of {files_inspected} examined in \
                     {scope}; {} file(s) were not eligible added layer-source subjects and were not \
                     measured. {}",
                    violations.len(),
                    files_inspected.saturating_sub(*files_classified),
                    violations
                        .iter()
                        .map(|v| format!("{}: {}", v.file_path, v.description))
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            }
        }
    };

    CleanArchitectureReport {
        is_clean,
        violations,
        summary,
        measurement,
        scope,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchViolation {
    pub file_path: String,
    pub source_layer: String, // "CORE/DOMAIN", "PORTS/APPLICATION"
    pub target_layer: String, // "ADAPTERS", "FACADE/REST"
    pub description: String,
    pub snippet: String,
}

/// Which architectural layer a file sits in, derived from its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchLayer {
    Core,
    Ports,
    Adapters,
    Facade,
}

/// Whether the guard was actually able to make an architectural claim.
///
/// Complete no-subject evidence differs from unavailable evidence; neither
/// is a measured clean result, but only the latter always blocks admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchMeasurement {
    /// At least one file belonged to a recognised layer and was checked.
    Measured {
        files_inspected: usize,
        files_classified: usize,
    },
    /// Complete observation found no applicable subject. Historical explicit
    /// records remain readable without inferring new semantics from prose.
    NotMeasured {
        reason: String,
        files_inspected: usize,
    },
    /// Acquisition or ownership evidence is incomplete, not an empty subject.
    Unavailable {
        reason: String,
        files_inspected: usize,
    },
}

impl Default for ArchMeasurement {
    /// Absent evidence, never a pass.
    fn default() -> Self {
        ArchMeasurement::Unavailable {
            reason: "no measurement recorded".to_string(),
            files_inspected: 0,
        }
    }
}

impl ArchMeasurement {
    pub fn is_measured(&self) -> bool {
        matches!(self, ArchMeasurement::Measured { .. })
    }

    /// `Some(reason)` when nothing could be measured.
    pub fn not_measured_reason(&self) -> Option<&str> {
        match self {
            ArchMeasurement::NotMeasured { reason, .. }
            | ArchMeasurement::Unavailable { reason, .. } => Some(reason),
            ArchMeasurement::Measured { .. } => None,
        }
    }

    pub fn files_inspected(&self) -> usize {
        match self {
            ArchMeasurement::Measured {
                files_inspected, ..
            } => *files_inspected,
            ArchMeasurement::NotMeasured {
                files_inspected, ..
            }
            | ArchMeasurement::Unavailable {
                files_inspected, ..
            } => *files_inspected,
        }
    }

    pub fn files_classified(&self) -> usize {
        match self {
            ArchMeasurement::Measured {
                files_classified, ..
            } => *files_classified,
            ArchMeasurement::NotMeasured { .. } | ArchMeasurement::Unavailable { .. } => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanArchitectureReport {
    /// True only when the run measured something *and* found no violations.
    /// An unmeasured run is never clean — check [`Self::measurement`] first.
    pub is_clean: bool,
    pub violations: Vec<ArchViolation>,
    pub summary: String,
    /// What the run was actually able to observe.
    #[serde(default)]
    pub measurement: ArchMeasurement,
    /// What was examined: a PR (`repo#number`) or a source tree path.
    #[serde(default)]
    pub scope: String,
}

impl CleanArchitectureReport {
    /// The production certification conversion; absence is never a pass.
    pub fn gate_status(&self) -> GateStatus {
        if !self.violations.is_empty() {
            return GateStatus::Failed(self.summary.clone());
        }
        if let ArchMeasurement::Unavailable { reason, .. } = &self.measurement {
            return GateStatus::Errored(reason.clone());
        }
        match self.measurement.not_measured_reason() {
            Some(reason) => GateStatus::NotMeasured {
                gate_id: "clean_arch_status".to_string(),
                reason: reason.to_string(),
            },
            None if self.is_clean => GateStatus::Passed,
            None => GateStatus::Failed(self.summary.clone()),
        }
    }
}
