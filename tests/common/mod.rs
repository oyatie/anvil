//! Shared by the docs-claim gate's two test binaries.
//!
//! Each integration test compiles its own copy, so an item used by only one of
//! them is dead in the other.
#![allow(dead_code)]

pub mod docs_claims;
