//! One bin of `AUDITED_GATES` entries.
//!
//! The split is by size, not by subject: `registry.rs` held every entry and
//! was 1166 lines against a 300-line budget. The order of the corpus lives in
//! `registry::AUDITED_GATES`, so nothing here is meant to be read as a grouping.

use crate::fidelity::{Fidelity, GateFidelity};

pub const SHUFFLE_STATUS: GateFidelity = GateFidelity {
    gate_id: "shuffle_status",
    aspiration: "Verify that the tenant-to-cell assignment in force gives every tenant a \
                 distinct shuffle shard, and that no two tenants share enough cells for one \
                 cell's failure to take both of them down.",
    reference: "AWS Builders' Library, Workload isolation using shuffle-sharding; Route 53 \
                infima; AWS cell-based architecture guidance",
    fidelity: Fidelity::Aspirational,
    gap: "Reads no tenant-to-cell mapping table, and a pull request diff carries none: the \
          assignment is control-plane state. The guard used to declare its own two-tenant \
          table, whose two shards shared exactly as many cells as the bound permitted, on \
          every pull request forever. That table is deleted and \
          evaluate_without_topology_source is the only path the pipeline takes \
          (shuffle_shard_simulator/mod.rs::ShuffleShardSimulator). The combinatorics survive as the seam a \
          real table plugs into -- calculate_combinations and evaluate_overlap are honest \
          (shuffle_shard_simulator/math.rs::ShuffleShardMath). What the gate published was also the wrong \
          quantity: cells per tenant over total cells is one tenant's infrastructure \
          footprint, and it rises as isolation improves. It is now \
          uniform_random_shard_collision_ratio, the reciprocal of the number of possible \
          shards that the infima javadoc defines as blast radius, and the name says \
          uniform_random because compute_metrics derives it from the two integers without \
          reading allocations at all \
          (shuffle_shard_simulator/math.rs::ShuffleShardMath). Checking a finished table is still weaker \
          than the oracle, which enforces the bound at assignment time with a sharder that \
          backtracks against every shard already handed out.",
    blocked_on: Some(
        "a tenant-to-cell mapping table, from a control plane or from a checked-in topology",
    ),
};

pub const PROGRESSIVE_RING_STATUS: GateFidelity = GateFidelity {
    gate_id: "progressive_ring_status",
    aspiration: "Advance a change through progressive-exposure rings only once the ring it \
                 occupies has baked for its declared minimum and no region pair is taking the \
                 rollout on both halves at once.",
    reference: "Azure Safe Deployment Practices; Azure Well-Architected OE:11 safe deployment; \
                Azure region pairs",
    fidelity: Fidelity::Aspirational,
    gap: "Deploys nothing and reads no cloud control plane, so the elapsed bake time and the \
          live region set are both unknown and evaluate_without_rollout_state is the path the \
          pipeline takes (progressive_rollout/mod.rs::evaluate_without_rollout_state). The health verdict used to be a \
          constant threaded through three calls and answered with the same literal in all four \
          arms of the scheduler; the field that carried it is gone, and the two validators \
          that check something real -- which had zero production callers -- are now reached \
          only through evaluate_ring_advance, which runs both \
          (progressive_rollout/mod.rs::evaluate_ring_advance). validate_bake_window compares \
          elapsed_bake_minutes against the manifest's own min_bake_minutes, and an undeclared \
          ring is no longer treated as satisfied \
          (progressive_rollout/ring_scheduler.rs::RingScheduler). compute_next_ring returns an \
          Option and holds the advance rather than reading an undeclared ring as \
          traffic_percentage zero, which was the same inversion one level up \
          (progressive_rollout/ring_scheduler.rs::RingScheduler). AZURE_REGION_PAIRS held region codes \
          lifted from a different cloud, paired by a rule Azure does not use -- it \
          pairs East US with West US, not with East US 2 -- and now holds the published table \
          (progressive_rollout/ring_scheduler.rs::AZURE_REGION_PAIRS). It stays partial: asymmetric pairs \
          and the growing set of nonpaired regions are not modelled, and a region it does not \
          name is treated as unpaired.",
    blocked_on: Some(
        "rollout state -- a bake clock over a deployed artefact, and the set of regions \
         currently taking the rollout",
    ),
};
