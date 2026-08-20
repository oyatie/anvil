//! Ports: the stable seam. The facade reaches the core only through here,
//! and adapters implement what is declared here. Everything is defined in
//! `core`; this module names the surface (facade -> ports -> core, ADR-0562
//! layering; `facade-core-layering` in oyatie).

pub use crate::shape::core::measure::measure;
pub use crate::shape::core::profile::LanguageProfile;
pub use crate::shape::core::report::{
    Finding, Fix, RuleId, ShapeDistance, ShapeReport, SpecSource, UnitConformance,
};
pub use crate::shape::core::resolve::{DiscoveryRule, ResolvedSpec, ResolvedUnit, resolve};
pub use crate::shape::core::spec::{RuleMode, SCHEMA_V1, ShapeSpec, SpecError};
pub use crate::shape::core::tree::{SourceError, TreeSource};
