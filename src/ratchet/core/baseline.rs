//! The baseline document.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const BASELINE_SCHEMA_V1: &str = "anvil/ratchet-baseline/v1";

/// How a rule's regressions are treated. Advisory rules count and report;
/// blocking rules fail on any key absent from the frozen baseline.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mode {
    #[serde(rename = "advisory-until-infra")]
    Advisory,
    #[serde(rename = "baseline-block-on-new")]
    BlockOnNew,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RuleBaseline {
    pub mode: Mode,
    /// A rule that may never accumulate a baseline: every key is a
    /// regression, at adoption and forever.
    #[serde(default)]
    pub frozen_empty: bool,
    #[serde(default)]
    pub keys: BTreeSet<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Baseline {
    pub schema: String,
    /// The commit the baseline was measured at — a full sha, so the number
    /// can be reproduced.
    pub measured_at: String,
    pub rules: BTreeMap<String, RuleBaseline>,
}

impl Baseline {
    pub fn parse(json: &[u8]) -> Result<Baseline, String> {
        let b: Baseline = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        if b.schema != BASELINE_SCHEMA_V1 {
            return Err(format!(
                "baseline schema must be {BASELINE_SCHEMA_V1:?}, got {:?}",
                b.schema
            ));
        }
        for (rule, rb) in &b.rules {
            if rb.frozen_empty && !rb.keys.is_empty() {
                return Err(format!(
                    "rule {rule} is frozen_empty but carries {} key(s)",
                    rb.keys.len()
                ));
            }
        }
        Ok(b)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Builds a baseline from measured keys and the modes the spec declares.
    /// A `frozen_empty` rule is recorded with no keys regardless of what was
    /// measured: its regressions are reported from day one.
    pub fn seed(
        measured_at: &str,
        keys_by_rule: &BTreeMap<String, BTreeSet<String>>,
        modes: &BTreeMap<String, (Mode, bool)>,
    ) -> Baseline {
        let mut rules = BTreeMap::new();
        for (rule, (mode, frozen_empty)) in modes {
            let keys = if *frozen_empty {
                BTreeSet::new()
            } else {
                keys_by_rule.get(rule).cloned().unwrap_or_default()
            };
            rules.insert(
                rule.clone(),
                RuleBaseline {
                    mode: *mode,
                    frozen_empty: *frozen_empty,
                    keys,
                },
            );
        }
        Baseline {
            schema: BASELINE_SCHEMA_V1.to_string(),
            measured_at: measured_at.to_string(),
            rules,
        }
    }

    pub fn total_keys(&self) -> usize {
        self.rules.values().map(|r| r.keys.len()).sum()
    }
}
