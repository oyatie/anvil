//! The seam: the facade imports the core only through here (facade ->
//! ports -> core), and the adapters that open branches and pull requests
//! will implement the traits declared here.

pub use crate::change_delivery::core::{
    Admission, DeliveryLedger, Held, LABEL_SHAPE_MOVE, LABEL_STRUCTURE_ONLY, LandingInputs,
    LandingMode, LandingPolicy, LedgerEntry, MOVE_PLAN_SCHEMA_V1, Move, MoveKind, NameStatus,
    OwnerMap, PurityViolation, Sequenced, ShapeMovePlan, Shard, ShardKey, ShardState, Withheld,
    admit, branch_name, conflict_pairs, diff_is_structure_only, pr_marker, select_independent,
    sequence, shard_key, shard_plan,
};

pub use crate::change_delivery::core::shard::occupancy::{
    Hop, SpawnKind, SpawnRefused, admit_in_queue, admit_spawn, ahead_of, anvil_hubs,
    path_sets_disjoint,
};

use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// One lane, one worktree (I19). The lease file inside it keeps the
/// worktree GC from reaping a live lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneWorktree {
    pub lane_id: String,
    pub path: PathBuf,
    /// The revision the lane was created at (full sha).
    pub base_rev: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaneError {
    Refused(String),
    Failed(String),
}

impl std::fmt::Display for LaneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LaneError::Refused(m) => write!(f, "refused: {m}"),
            LaneError::Failed(m) => write!(f, "failed: {m}"),
        }
    }
}

impl std::error::Error for LaneError {}

/// Worktree-level version control operations for a lane. No implementation
/// may force-push, skip hooks, or push at all — pushing is the landing
/// step's job and lands in its own change.
#[async_trait]
pub trait VcsPort: Send + Sync {
    /// Creates a detached lane worktree at `base_rev`. Refuses the daemon's
    /// own source tree unless `allow_same_repo` (dry-runs only).
    async fn create_lane(
        &self,
        repo_dir: &Path,
        lane_id: &str,
        base_rev: &str,
        allow_same_repo: bool,
    ) -> Result<LaneWorktree, LaneError>;
    async fn apply_move(&self, lane: &LaneWorktree, from: &str, to: &str) -> Result<(), LaneError>;
    /// Stages everything except Anvil's own receipt paths.
    async fn stage(&self, lane: &LaneWorktree) -> Result<(), LaneError>;
    async fn name_status(&self, lane: &LaneWorktree) -> Result<Vec<NameStatus>, LaneError>;
    async fn cached_diff(&self, lane: &LaneWorktree) -> Result<String, LaneError>;
    async fn diffstat(&self, lane: &LaneWorktree) -> Result<String, LaneError>;
    async fn cleanup(&self, lane: LaneWorktree) -> Result<(), LaneError>;
}

/// Applies a shard's moves inside the lane. An engine refuses what it cannot
/// do mechanically; a refusal fails the shard, it never falls through to a
/// broader tool.
#[async_trait]
pub trait RewriteEngine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn apply(
        &self,
        vcs: &dyn VcsPort,
        lane: &LaneWorktree,
        shard: &Shard,
    ) -> Result<(), LaneError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResult {
    Passed {
        label: String,
    },
    Failed {
        label: String,
        why: String,
    },
    /// No gate this build can run here — never a pass (I1).
    Unavailable {
        reason: String,
    },
}

#[async_trait]
pub trait LocalGate: Send + Sync {
    async fn run(&self, lane: &LaneWorktree) -> GateResult;
}
