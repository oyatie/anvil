//! Entry points that callers use: the CLI today; the certification gate and
//! the fleet sweep as they land.

/// Reading a repository tree at a revision, for callers outside this unit.
pub use crate::shape::adapters::GitTreeAtRev;
pub use crate::shape::core::tree::TreeSource;

/// Which build system marks a unit. Consumers ask shape what a unit marker is
/// rather than spelling `Cargo.toml` or `BUCK` themselves, which is the rule
/// I13 enforces; the facade has to name it for that to be possible.
pub use crate::shape::ports::LanguageProfile;

/// The measurement a consumer receives, and the repair it may carry.
///
/// Withheld until now for a real reason: `shape::facade::sweep` imported
/// `change_delivery`, so exporting these would have made an edge legal inside
/// a two-member cycle that no per-edge rule can see. That cycle is broken —
/// the sweep measures and the composition root plans — so delivery consuming
/// a report is now the only direction that exists, and shape should name it.
pub use crate::shape::ports::{Fix, ShapeReport};

pub mod admit;
pub mod baseline;
pub mod cli;
pub mod gate;
pub mod measure;
pub mod sweep;
