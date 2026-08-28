/// The landing policy a tenant carries, as callers outside this unit read it.
///
/// `LandingPolicy::load` is how a caller turns tenant bytes into a policy; a
/// consumer that must reach into `core` for it is being charged for a door the
/// unit failed to open.
pub use crate::change_delivery::core::LandingPolicy;

/// Where a lane records that it holds a worktree.
///
/// `git_manager` cleans up abandoned lanes and must know the marker's name; a
/// consumer forced into `adapters` for a constant is charged for a door the
/// unit did not open.
pub use crate::change_delivery::adapters::git_vcs::LANE_LEASE_FILE;

pub mod deliver;
pub mod occupancy;
pub mod plan;
