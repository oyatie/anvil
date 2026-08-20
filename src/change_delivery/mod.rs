//! Change delivery: turning a measured move plan into small, owner-disjoint,
//! structure-only pull requests, and landing them under a per-repository
//! policy — slowly and surely.
//!
//! This module holds the pure core and the dry-run facade. The ports and
//! adapters that open branches and pull requests land separately, so every
//! decision here (what goes in a shard, whether a diff is pure, whether a
//! shard may be enlisted) is testable without a repository or a network.

pub mod core;
pub mod facade;
