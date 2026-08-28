//! Entry points that callers use: the CLI today; the certification gate and
//! the fleet sweep as they land.

/// Reading a repository tree at a revision, for callers outside this unit.
pub use crate::shape::adapters::GitTreeAtRev;
pub use crate::shape::core::tree::TreeSource;

pub mod admit;
pub mod baseline;
pub mod cli;
pub mod gate;
pub mod measure;
pub mod sweep;
