//! Path occupancy, as the rest of the tree may use it.
//!
//! `core::occupancy` decides whether one write-set may start given what is
//! already in flight, including the hub rule: a write touching a hub file goes
//! alone and only from trunk HEAD. Exposing it here is what lets a scheduler
//! use that decision instead of reimplementing set intersection and silently
//! losing the hub half.
//!
//! `admit_in_queue` is the same decision taken against the hops *ahead* of one
//! hop rather than against all of them, which is what turns an overlapping pair
//! from a standoff into an order.

pub use crate::change_delivery::ports::{
    Hop, SpawnKind, SpawnRefused, admit_in_queue, admit_spawn, ahead_of, anvil_hubs,
    path_sets_disjoint,
};
