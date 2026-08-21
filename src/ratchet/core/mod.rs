//! Pure ratchet model: baseline, sign-off, comparison, monotonicity.

pub mod baseline;
pub mod compare;
pub mod signoff;

pub use baseline::{BASELINE_SCHEMA_V1, Baseline, Mode, RuleBaseline};
pub use compare::{Growth, RatchetVerdict, RuleVerdict, compare, regen_is_monotonic};
pub use signoff::{SIGNOFF_SCHEMA_V1, Signing, Signoff};
