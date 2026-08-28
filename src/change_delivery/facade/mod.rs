/// The landing policy a tenant carries, as callers outside this unit read it.
///
/// `LandingPolicy::load` is how a caller turns tenant bytes into a policy; a
/// consumer that must reach into `core` for it is being charged for a door the
/// unit failed to open.
pub use crate::change_delivery::core::LandingPolicy;

pub mod deliver;
pub mod occupancy;
pub mod plan;
