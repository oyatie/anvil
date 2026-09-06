//! The frozen reference: the baseline and signoff as committed at the
//! merge-base of a change. Adapters fetch it; core only compares.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefError {
    Unavailable(String),
}

impl std::fmt::Display for RefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefError::Unavailable(e) => write!(f, "frozen reference unavailable: {e}"),
        }
    }
}

impl std::error::Error for RefError {}

pub trait FrozenReferenceSource {
    /// The commit the reference is read from.
    fn reference_rev(&self) -> &str;
    /// Bytes of `path` at the reference, `None` when absent there.
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, RefError>;
}

pub use crate::ratchet::core::{
    BASELINE_SCHEMA_V1, Baseline, Growth, Mode, RatchetVerdict, RuleBaseline, RuleVerdict,
    SIGNOFF_SCHEMA_V1, Signing, Signoff, compare, regen_is_monotonic,
};
