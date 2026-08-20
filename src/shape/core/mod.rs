//! Pure shape model: the spec, its validation and resolution, language
//! profiles, and the report vocabulary. Nothing here touches the filesystem,
//! git, a clock or a subprocess.

pub mod profile;
pub mod report;
pub mod resolve;
pub mod spec;
pub mod validate;

pub use profile::LanguageProfile;
pub use report::{Finding, Fix, RuleId, ShapeDistance, ShapeReport, SpecSource, UnitConformance};
pub use resolve::{DiscoveryRule, ResolvedSpec, ResolvedUnit, resolve};
pub use spec::{RuleMode, SCHEMA_V1, ShapeSpec, SpecError};
pub use validate::validate;
