//! Shape Program: measure a repository against its tenant-carried shape
//! specification and emit the distance, the findings, and the moves that
//! close them.
//!
//! Anvil's source carries no layout of its own (I13). Every unit name, face
//! directory, satellite class, naming rule and placement step comes from the
//! spec the tenant repository commits — `.anvil/shape.json` — so the same
//! engine measures oyatie, console and Anvil itself, and a rule Anvil cannot
//! state generically is a rule that belongs in the tenant, not the tool.
//!
//! Faces: `core` is pure (no IO, no clocks); `facade` is the surface the CLI,
//! the certification gate and the fleet sweep call.

pub mod adapters;
pub mod core;
pub mod facade;
pub mod ports;
