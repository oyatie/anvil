//! Loading a frozen reference into a `Baseline` + `Signoff` pair. Absence of a
//! baseline at the reference is reported as such; it is never treated as an
//! empty baseline (which would make every existing key a regression) nor as
//! a pass.

use crate::ratchet::ports::{Baseline, FrozenReferenceSource, RefError, Signoff};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    /// Both documents parsed (signoff defaults to empty when absent).
    Frozen {
        rev: String,
        baseline: Baseline,
        signoff: Signoff,
    },
    /// No baseline at the reference: the change that introduces one.
    Bootstrap { rev: String },
}

pub fn load_reference(
    source: &dyn FrozenReferenceSource,
    baseline_path: &str,
    signoff_path: &str,
) -> Result<Reference, RefError> {
    let rev = source.reference_rev().to_string();
    let Some(raw) = source.read(baseline_path)? else {
        return Ok(Reference::Bootstrap { rev });
    };
    let baseline = Baseline::parse(&raw)
        .map_err(|e| RefError::Unavailable(format!("{baseline_path} at {rev}: {e}")))?;
    let signoff = match source.read(signoff_path)? {
        Some(raw) => Signoff::parse(&raw)
            .map_err(|e| RefError::Unavailable(format!("{signoff_path} at {rev}: {e}")))?,
        None => Signoff::default(),
    };
    Ok(Reference::Frozen {
        rev,
        baseline,
        signoff,
    })
}
