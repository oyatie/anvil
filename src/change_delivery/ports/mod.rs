//! The seam: the facade imports the core only through here (facade ->
//! ports -> core), and the adapters that open branches and pull requests
//! will implement the traits declared here.

pub use crate::change_delivery::core::{
    Admission, DeliveryLedger, LABEL_SHAPE_MOVE, LABEL_STRUCTURE_ONLY, LandingInputs, LandingMode,
    LandingPolicy, LedgerEntry, MOVE_PLAN_SCHEMA_V1, Move, MoveKind, NameStatus, OwnerMap,
    PurityViolation, ShapeMovePlan, Shard, ShardKey, ShardState, Withheld, admit, branch_name,
    conflict_pairs, diff_is_structure_only, pr_marker, select_independent, shard_key, shard_plan,
};
