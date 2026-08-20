//! The move plan (what the measurement asks for) and the shard (what one
//! pull request carries).

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MOVE_PLAN_SCHEMA_V1: &str = "anvil/move-plan/v1";

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum MoveKind {
    MoveFile,
    MoveDir,
    RenameCrate,
    CreateSkeleton,
    SplitSatellite,
    AddManifest,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Move {
    pub kind: MoveKind,
    pub from: String,
    pub to: String,
    pub unit: String,
    pub rule_id: String,
    pub evidence: String,
    /// The path the finding was raised on (a manifest for a crate rename,
    /// the unit root for a skeleton); used to resolve owners and touch sets
    /// when `from`/`to` are names rather than paths.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    pub destination_stable: bool,
    /// Lower ranks first: stable units, then mechanical satellite moves, then
    /// single files, then crate moves.
    pub rank: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ShapeMovePlan {
    pub schema: String,
    pub repo: String,
    /// The measured revision the plan was derived from.
    pub rev: String,
    /// Identity of the spec the plan was measured against; a plan from a
    /// different spec version is a different plan.
    pub spec_version: String,
    pub moves: Vec<Move>,
}

impl ShapeMovePlan {
    pub fn parse(json: &[u8]) -> Result<Self, String> {
        let p: ShapeMovePlan = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        if p.schema != MOVE_PLAN_SCHEMA_V1 {
            return Err(format!(
                "move plan schema must be {MOVE_PLAN_SCHEMA_V1:?}, got {:?}",
                p.schema
            ));
        }
        Ok(p)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShardKey(pub String);

impl std::fmt::Display for ShardKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One pull request's worth of moves: one unit, one rule.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Shard {
    pub key: ShardKey,
    pub repo: String,
    pub unit: String,
    pub rule_id: String,
    pub spec_version: String,
    pub moves: Vec<Move>,
    /// Moved paths plus the wiring files the rewrite is predicted to touch.
    pub touch_set: BTreeSet<String>,
    pub owners: BTreeSet<String>,
    pub touches_hot_file: bool,
    pub destination_stable: bool,
    pub generation: u32,
}

impl Shard {
    pub fn rank(&self) -> u32 {
        self.moves.iter().map(|m| m.rank).min().unwrap_or(u32::MAX)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ShardState {
    Planned,
    Opened { pr: u64 },
    Green { pr: u64 },
    Enlisted { pr: u64 },
    Merged { pr: u64, sha: String },
    Superseded { by: ShardKey },
    DismissedByHuman { pr: u64 },
    Reverted { pr: u64, revert_pr: u64 },
    Failed { reason: String },
}
