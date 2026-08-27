//! The shape of a clean-architecture verdict.
//!
//! Split out of one 928-line file under ADR-0719 D-35: 300 physical lines is
//! the born-blocking maximum for a hand-written file, and a split is more
//! `snake_case.rs` modules inside the same crate -- not a new crate, and not
//! a face taxonomy, which D-8 forbids a unit from inventing.

use serde::{Deserialize, Serialize};

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
/// The third state matters: a scan that classified zero files did not verify
/// anything, and must not collapse into "clean".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchMeasurement {
    /// At least one file belonged to a recognised layer and was checked.
    Measured {
        files_inspected: usize,
        files_classified: usize,
    },
    /// Nothing to measure: no file in the input belonged to core/ports/
    /// adapters/facade, or the tree could not be read. No claim is made in
    /// either direction.
    NotMeasured {
        reason: String,
        files_inspected: usize,
    },
}

impl Default for ArchMeasurement {
    /// Absent evidence, never a pass.
    fn default() -> Self {
        ArchMeasurement::NotMeasured {
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
            ArchMeasurement::NotMeasured { reason, .. } => Some(reason),
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
            } => *files_inspected,
        }
    }

    pub fn files_classified(&self) -> usize {
        match self {
            ArchMeasurement::Measured {
                files_classified, ..
            } => *files_classified,
            ArchMeasurement::NotMeasured { .. } => 0,
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
