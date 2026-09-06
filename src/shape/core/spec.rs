//! The shape specification as data. Field names are the JSON keys; every
//! struct rejects unknown fields so a typo in a tenant's spec is an error,
//! not a silently-ignored rule.

use super::profile::LanguageProfile;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_V1: &str = "anvil/shape/v1";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ShapeSpec {
    pub schema: String,
    #[serde(default)]
    pub destination_stable_default: bool,
    pub profiles: Vec<LanguageProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_registry: Option<UnitRegistryRef>,
    pub unit_kinds: BTreeMap<String, UnitKind>,
    pub skeletons: BTreeMap<String, Skeleton>,
    #[serde(default)]
    pub placement: Placement,
    pub root_files: RootFiles,
    #[serde(default)]
    pub naming: Naming,
    #[serde(default)]
    pub legacy_roots: Vec<String>,
    #[serde(default)]
    pub units: BTreeMap<String, UnitOverride>,
    pub rules: BTreeMap<String, RuleConfig>,
}

/// A JSON document in the tenant repository that lists units (and optionally
/// meta directories and faces). `pointer` is an RFC 6901 JSON pointer to a
/// list; `key` names the field that carries each element's name.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitRegistryRef {
    pub path: String,
    pub units: JsonList,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta_dirs: Option<JsonList>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faces: Option<JsonList>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JsonList {
    pub pointer: String,
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitKind {
    /// Root pattern with `<name>` standing for the unit name, e.g. `"<name>/"`.
    pub root: String,
    pub skeleton: String,
    /// `"registry"`, `"registry-meta-dirs"`, or `"discover:<marker>"`.
    pub members: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MembersSource {
    Registry,
    RegistryMetaDirs,
    Discover { marker: String },
}

impl UnitKind {
    pub fn members_source(&self) -> Result<MembersSource, String> {
        match self.members.as_str() {
            "registry" => Ok(MembersSource::Registry),
            "registry-meta-dirs" => Ok(MembersSource::RegistryMetaDirs),
            other => match other.strip_prefix("discover:") {
                Some(marker) if !marker.trim().is_empty() => Ok(MembersSource::Discover {
                    marker: marker.trim().to_string(),
                }),
                _ => Err(format!(
                    "members must be \"registry\", \"registry-meta-dirs\" or \"discover:<marker>\", got {other:?}"
                )),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Skeleton {
    /// Face name -> directory under the unit root.
    pub faces: BTreeMap<String, String>,
    #[serde(default)]
    pub required_faces: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_marker: Option<String>,
    /// The Dependency Rule as data: face -> faces it may depend on. A face
    /// absent from the map, or absent from its own list, may depend on nothing.
    #[serde(default)]
    pub face_dependency_matrix: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub cross_unit_edges: CrossUnitEdges,
    /// The declared satellite set: class -> canonical home and form.
    #[serde(default)]
    pub satellites: BTreeMap<String, Satellite>,
    #[serde(default)]
    pub allowed_unit_root_files: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossUnitEdges {
    #[default]
    #[serde(rename = "facade-only")]
    FacadeOnly,
    #[serde(rename = "any")]
    Any,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Satellite {
    pub dir: String,
    pub form: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub steps: Vec<PlacementStep>,
}

impl Default for Placement {
    fn default() -> Self {
        Placement {
            steps: vec![PlacementStep::UnitByFace],
        }
    }
}

/// ADR-0562 §3 transcribed as data: first match wins.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PlacementStep {
    MetaDir {
        dir: String,
    },
    ArtifactClass {
        class: String,
        dest: String,
        #[serde(default)]
        patterns: Vec<String>,
    },
    SharedPrimitive {
        min_consumers: u32,
        #[serde(default)]
        below_all: bool,
        dest: String,
    },
    Composition {
        min_units: u32,
        dest: String,
    },
    UnitByFace,
    LowestDagNode,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RootFiles {
    pub mode: RootFilesMode,
    pub rules: Vec<RootFileRule>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RootFilesMode {
    Allowlist,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RootFileRule {
    pub id: String,
    pub kind: RootRuleKind,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RootRuleKind {
    Exact,
    Prefix,
    Suffix,
    /// `README` matches `README`, `README.md`, `README.adoc`.
    PrefixDot,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Naming {
    #[serde(default)]
    pub crate_prefix: String,
    #[serde(default)]
    pub layer_suffixes: Vec<String>,
    #[serde(default)]
    pub face_by_suffix: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter_name: Option<String>,
    #[serde(default)]
    pub transient_adapter_suffixes: Vec<String>,
    #[serde(default)]
    pub port_name_suffixes: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct UnitOverride {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination_stable: Option<bool>,
    /// Satellite classes this unit legitimately does not carry. An absent
    /// satellite is otherwise a finding; an empty placeholder is never wanted.
    #[serde(default)]
    pub satellites_not_applicable: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuleConfig {
    pub mode: RuleMode,
    #[serde(default)]
    pub frozen_empty: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleMode {
    #[serde(rename = "advisory-until-infra")]
    AdvisoryUntilInfra,
    #[serde(rename = "baseline-block-on-new")]
    BaselineBlockOnNew,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    Parse(String),
    Invalid(Vec<String>),
    Registry(String),
}

impl std::fmt::Display for SpecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecError::Parse(e) => write!(f, "shape spec does not parse: {e}"),
            SpecError::Invalid(problems) => {
                writeln!(f, "shape spec is invalid ({} problem(s)):", problems.len())?;
                for p in problems {
                    writeln!(f, "  - {p}")?;
                }
                Ok(())
            }
            SpecError::Registry(e) => write!(f, "unit registry could not be used: {e}"),
        }
    }
}

impl std::error::Error for SpecError {}

impl ShapeSpec {
    /// Parses and validates. A spec that parses but breaks its own invariants
    /// is `Invalid`, listing every problem rather than the first.
    pub fn parse(json: &str) -> Result<Self, SpecError> {
        let spec: ShapeSpec =
            serde_json::from_str(json).map_err(|e| SpecError::Parse(e.to_string()))?;
        let problems = super::validate::validate(&spec);
        if problems.is_empty() {
            Ok(spec)
        } else {
            Err(SpecError::Invalid(problems))
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}
