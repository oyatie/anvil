//! What Anvil remembers about its own shape PRs. A cache: the branch on
//! GitHub is the source of truth, so a lost ledger can cost a lookup but
//! never produce a duplicate.

use super::model::{ShardKey, ShardState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct LedgerEntry {
    pub branch: String,
    pub generation: u32,
    pub spec_version: String,
    pub state: ShardState,
    pub updated: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct DeliveryLedger {
    pub repo: String,
    pub entries: BTreeMap<ShardKey, LedgerEntry>,
}

impl DeliveryLedger {
    pub fn parse(json: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(json).map_err(|e| e.to_string())
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn open_count(&self) -> u32 {
        self.entries
            .values()
            .filter(|e| {
                matches!(
                    e.state,
                    ShardState::Opened { .. }
                        | ShardState::Green { .. }
                        | ShardState::Enlisted { .. }
                )
            })
            .count() as u32
    }

    /// A human closed it: never reopened until the spec version changes.
    pub fn dismissed_for(&self, key: &ShardKey, spec_version: &str) -> bool {
        self.entries.get(key).is_some_and(|e| {
            matches!(e.state, ShardState::DismissedByHuman { .. }) && e.spec_version == spec_version
        })
    }

    pub fn transition(
        &mut self,
        key: &ShardKey,
        state: ShardState,
        now: &str,
    ) -> Result<(), String> {
        let Some(e) = self.entries.get_mut(key) else {
            return Err(format!("unknown shard {key}"));
        };
        let ok = matches!(
            (&e.state, &state),
            (ShardState::Planned, ShardState::Opened { .. })
                | (ShardState::Planned, ShardState::Failed { .. })
                | (ShardState::Opened { .. }, ShardState::Green { .. })
                | (
                    ShardState::Opened { .. },
                    ShardState::DismissedByHuman { .. }
                )
                | (ShardState::Opened { .. }, ShardState::Superseded { .. })
                | (ShardState::Opened { .. }, ShardState::Failed { .. })
                | (ShardState::Green { .. }, ShardState::Enlisted { .. })
                | (ShardState::Green { .. }, ShardState::Merged { .. })
                | (ShardState::Green { .. }, ShardState::Opened { .. })
                | (
                    ShardState::Green { .. },
                    ShardState::DismissedByHuman { .. }
                )
                | (ShardState::Green { .. }, ShardState::Superseded { .. })
                | (ShardState::Enlisted { .. }, ShardState::Merged { .. })
                | (ShardState::Enlisted { .. }, ShardState::Green { .. })
                | (ShardState::Enlisted { .. }, ShardState::Superseded { .. })
                | (ShardState::Merged { .. }, ShardState::Reverted { .. })
        );
        if !ok {
            return Err(format!(
                "illegal transition for {key}: {:?} -> {:?}",
                e.state, state
            ));
        }
        e.state = state;
        e.updated = now.to_string();
        Ok(())
    }
}
