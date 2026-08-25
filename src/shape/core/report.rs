//! Report vocabulary. A `ShapeReport` says what was measured, what was not
//! and why, and what would close each finding. Counts are derived from the
//! findings, never stored separately (I2).

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(pub String);

impl RuleId {
    pub fn new(id: &str) -> Self {
        RuleId(id.to_string())
    }
}

impl std::fmt::Display for RuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where the spec the measurement used came from. A number measured against
/// a proposed spec is never presented as adopted.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SpecSource {
    /// `.anvil/shape.json` read from the tree under evaluation.
    Adopted,
    /// A spec supplied from outside the tree (CLI `--spec-override`, fixtures).
    Proposed(String),
    /// The PR that introduces the spec: no spec existed at the merge-base.
    CandidateBootstrap,
}

/// The machine-applicable remedy that ships with a finding.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Fix {
    Move {
        from: String,
        to: String,
    },
    Rename {
        from: String,
        to: String,
    },
    DependOnInstead {
        replace: String,
        with: String,
    },
    /// Create a path that should exist.
    ///
    /// `content` is what makes this applicable rather than merely reportable:
    /// a fix naming a path with nothing to put in it can be printed but not
    /// executed, and a scaffold that creates empty files is not a scaffold.
    /// `None` means the path is required and the content needs judgement --
    /// which is a finding a human must close, not one a codemod can.
    Create {
        path: String,
        content: Option<String>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: RuleId,
    /// Stable identity for the ratchet: a crate name for dependency/naming
    /// rules, a path for placement rules (the move is the shrink).
    pub key: String,
    pub path: String,
    pub unit: Option<String>,
    pub detail: String,
    pub fix: Option<Fix>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct UnitConformance {
    pub unit: String,
    pub kind: String,
    pub faces_present: Vec<String>,
    pub faces_missing: Vec<String>,
    pub satellites_aliased: Vec<String>,
    pub destination_stable: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PlannedMove {
    pub from: String,
    pub to: String,
    pub rule: RuleId,
    pub unit: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct ShapeDistance {
    pub findings_total: usize,
    pub units_total: usize,
    pub units_conformant: usize,
    pub files_misplaced: usize,
    pub edges_denied: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShapeReport {
    pub repo: String,
    pub rev: String,
    pub spec_source: SpecSource,
    pub units: Vec<UnitConformance>,
    pub findings: Vec<Finding>,
    /// Rules that could not be measured, with the reason (I1: these are never
    /// counted as clean).
    pub not_measured: Vec<(RuleId, String)>,
}

impl ShapeReport {
    pub fn distance(&self) -> ShapeDistance {
        ShapeDistance {
            findings_total: self.findings.len(),
            units_total: self.units.len(),
            units_conformant: self
                .units
                .iter()
                .filter(|u| u.faces_missing.is_empty() && u.satellites_aliased.is_empty())
                .count(),
            files_misplaced: self
                .findings
                .iter()
                .filter(|f| matches!(f.fix, Some(Fix::Move { .. })))
                .count(),
            edges_denied: self
                .findings
                .iter()
                .filter(|f| matches!(f.fix, Some(Fix::DependOnInstead { .. })))
                .count(),
        }
    }

    pub fn move_plan(&self) -> Vec<PlannedMove> {
        self.findings
            .iter()
            .filter_map(|f| match &f.fix {
                Some(Fix::Move { from, to }) => Some(PlannedMove {
                    from: from.clone(),
                    to: to.clone(),
                    rule: f.rule.clone(),
                    unit: f.unit.clone(),
                }),
                _ => None,
            })
            .collect()
    }
}
