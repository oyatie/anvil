//! Freshness supplied by an adapter; this core does not inspect Git or identity.
use super::{Hop, SpawnKind, SpawnRefused, admit_with_freshness, ahead_of};
use std::collections::BTreeSet;

/// The applicable hub freshness fact. Promotion equivalence requires a verified
/// same-repository predecessor and equal complete destination/merge-base trees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubBaseFreshness {
    AtDestinationTip,
    EquivalentPromotionTree,
    Stale,
}

/// Legacy boolean callers supply ancestry only, never promotion authority.
impl From<bool> for HubBaseFreshness {
    fn from(at_tip: bool) -> Self {
        if at_tip {
            Self::AtDestinationTip
        } else {
            Self::Stale
        }
    }
}

/// The same overlap/N=1 decision, using an explicitly supplied freshness fact.
pub fn admit_in_queue_with_freshness(
    write: &BTreeSet<String>,
    position: u64,
    hubs: &BTreeSet<String>,
    open: &[Hop],
    freshness: HubBaseFreshness,
) -> Result<SpawnKind, SpawnRefused> {
    let ahead: Vec<BTreeSet<String>> = ahead_of(position, open)
        .into_iter()
        .map(|h| h.write.clone())
        .collect();
    admit_with_freshness(write, hubs, &ahead, freshness)
}
