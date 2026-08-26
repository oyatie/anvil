//! Pure shape model: the spec, its validation and resolution, language
//! profiles, and the report vocabulary. Nothing here touches the filesystem,
//! git, a clock or a subprocess.

pub mod dependency;
pub mod graph_shape;
pub mod glob;
pub mod measure;
pub mod naming;
pub mod placement;
pub mod profile;
pub mod report;
pub mod resolve;
pub mod root_hygiene;
pub mod skeleton;
pub mod spec;
pub mod tree;
pub mod validate;

pub use dependency::{DepEdge, DepGraph, classify};
pub use measure::measure;
pub use placement::{DepFacts, PathFacts, Placement, RoleFacts, place};
pub use profile::LanguageProfile;
pub use report::{Finding, Fix, RuleId, ShapeDistance, ShapeReport, SpecSource, UnitConformance};
pub use resolve::{DiscoveryRule, ResolvedSpec, ResolvedUnit, resolve};
pub use spec::{RuleMode, SCHEMA_V1, ShapeSpec, SpecError};
pub use validate::validate;
