//! Path occupancy, as the rest of the tree may use it.
//!
//! `core::occupancy` decides whether one write-set may start given what is
//! already in flight, including the hub rule: a write touching a hub file goes
//! alone and only from trunk HEAD. Exposing it here is what lets a scheduler
//! use that decision instead of reimplementing set intersection and silently
//! losing the hub half.

pub use crate::change_delivery::core::shard::occupancy::{
    SpawnKind, SpawnRefused, admit_spawn, anvil_hubs, path_sets_disjoint,
};
