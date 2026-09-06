//! The one-way door. The only ratchet file a human edits: a key listed here
//! is exempt from the shrink-only rule for exactly one regeneration, after
//! which it is part of the baseline and the entry is inert (and fails).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const SIGNOFF_SCHEMA_V1: &str = "anvil/ratchet-signoff/v1";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Signing {
    pub by: String,
    pub date: String,
    pub note: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct Signoff {
    #[serde(default = "default_schema")]
    pub schema: String,
    /// rule -> keys a human accepts into the baseline on the next regen.
    #[serde(default, rename = "_sign_off_additions")]
    pub additions: BTreeMap<String, BTreeSet<String>>,
    /// Rules a human allows to move from block to advisory on the next regen
    /// (G12 applied to the mode field: a downgrade is growth).
    #[serde(default, rename = "_mode_downgrades")]
    pub mode_downgrades: BTreeSet<String>,
    #[serde(default)]
    pub signings: Vec<Signing>,
}

fn default_schema() -> String {
    SIGNOFF_SCHEMA_V1.to_string()
}

impl Signoff {
    pub fn parse(json: &[u8]) -> Result<Signoff, String> {
        let s: Signoff = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        if s.schema != SIGNOFF_SCHEMA_V1 {
            return Err(format!(
                "signoff schema must be {SIGNOFF_SCHEMA_V1:?}, got {:?}",
                s.schema
            ));
        }
        let has_entries = !s.additions.is_empty() || !s.mode_downgrades.is_empty();
        if has_entries && s.signings.is_empty() {
            return Err("signoff carries entries but no signing".to_string());
        }
        Ok(s)
    }

    pub fn is_empty(&self) -> bool {
        self.additions.values().all(BTreeSet::is_empty) && self.mode_downgrades.is_empty()
    }

    pub fn covers(&self, rule: &str, key: &str) -> bool {
        self.additions.get(rule).is_some_and(|k| k.contains(key))
    }
}
