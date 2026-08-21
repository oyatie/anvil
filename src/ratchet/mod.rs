//! Generic shrink-only ratchet (G15, I7).
//!
//! A baseline records, per rule, the keys that were already failing when the
//! rule was adopted. Thereafter a key may leave the baseline (fixed) but may
//! not enter it (regression) unless a human signs it off — once, visibly, in
//! a file that is otherwise never edited by hand. The reference that a change
//! is judged against is the baseline as committed at the merge-base of the
//! change, never the copy in the change itself, so a change cannot launder
//! its own regressions.
//!
//! Transcribed from oyatie's `ci/facade/baseline-ratchet`: frozen reference
//! at merge-base, per-rule mode flipped by data, `frozen_empty` rules that can
//! never accumulate, a one-way sign-off door whose inert entries fail.
//!
//! The ratchet knows nothing about shape: keys and rule ids are strings.

pub mod adapters;
pub mod core;
pub mod facade;
pub mod ports;
