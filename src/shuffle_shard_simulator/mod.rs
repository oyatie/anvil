//! Cell shuffle sharding — the gate that wrote the topology it judged.
//!
//! # What was here
//!
//! `evaluate_shuffle_sharding` declared a cell count, a per-tenant shard size
//! and a two-tenant assignment table inside the gate, then measured that table
//! against a bound. The two shards it wrote shared exactly two cells and the
//! bound was two, on every pull request, forever. The pull request itself
//! reached the gate only as a field in a log line.
//!
//! The number it published was worse than unmoving, it was the wrong quantity:
//! "blast radius limited to 50.0%" was the per-tenant shard size divided by the
//! cell count. See [`math::BlastRadiusMetrics::full_shard_overlap_ratio`] for
//! why that inverts the sign of the claim.
//!
//! # What is here now
//!
//! Nothing is fabricated. A tenant-to-cell assignment is control-plane state —
//! the AWS cell-based architecture guidance is explicit that "the task of
//! mapping a new customer to a cell and registering it in the cell router is
//! the control plane's task" — and nothing here reads one, so the gate reports
//! `GateStatus::NotMeasured` naming that. A pull request diff carries no tenant
//! table, and a topology invented to fill the gap is the defect again.
//!
//! [`ShuffleShardMath`] is retained and still exported. It is real
//! combinatorics — C(n,k) over the supplied cell count, and the true maximum
//! pairwise intersection over the supplied assignment — and it is the seam a
//! mapping table plugs into. [`ShuffleShardMath::select_tenant_cells`] is the
//! hash-based sharder, the shape infima calls `SimpleSignatureShuffleSharder`.
//!
//! # Distance from the oracle, beyond the missing table
//!
//! The bound this gate enforces is Route 53's real guarantee — no two customer
//! domains share more than two virtual name servers — but Route 53 enforces it
//! *at assignment time*, with a searching sharder that backtracks against a
//! store of every shard already handed out. Checking a finished table after the
//! fact tells you the invariant held; it cannot make it hold.

use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::report::GateStatus;

pub mod math;
pub use math::{BlastRadiusMetrics, ShuffleShardAllocation, ShuffleShardMath};

/// Matches the `PreMergeCertificationReport` field name.
const GATE_ID: &str = "shuffle_status";

/// What must exist before an overlap bound can be checked at all.
const MISSING_TOPOLOGY_SOURCE: &str = "no tenant-to-cell mapping table is read from a control plane or from a checked-in \
     topology, so no tenant assignment exists to compute pairwise shard overlap or \
     blast radius over";

/// Why a supplied table can still yield no measurement: an overlap bound is a
/// property of a PAIR of shards.
const NO_TENANT_PAIR: &str = "the supplied topology assigns fewer than two tenants, so no pair of shards was \
     compared and the overlap bound was never tested against anything";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShuffleShardReport {
    pub status: GateStatus,
    /// The topology the verdict describes. `None` when nothing was judged.
    pub metrics: Option<BlastRadiusMetrics>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ShuffleShardSimulator;

impl ShuffleShardSimulator {
    pub fn new() -> Self {
        Self
    }

    /// Judges a caller-supplied tenant-to-cell assignment against a
    /// caller-supplied bound on how many cells any two tenants may share.
    pub fn evaluate_topology(
        &self,
        total_cells: usize,
        cells_per_tenant: usize,
        allocations: &[ShuffleShardAllocation],
        max_tenant_overlap: usize,
    ) -> ShuffleShardReport {
        if cells_per_tenant == 0 || cells_per_tenant > total_cells {
            return ShuffleShardReport {
                status: GateStatus::Errored(format!(
                    "supplied topology is not realisable: {cells_per_tenant} cells per tenant \
                     out of {total_cells} cells, so no shuffle shard can be drawn and there is \
                     no blast radius to report"
                )),
                metrics: None,
            };
        }

        if allocations.len() < 2 {
            return Self::not_measured(NO_TENANT_PAIR);
        }

        let metrics = ShuffleShardMath::compute_metrics(total_cells, cells_per_tenant, allocations);

        let status = if metrics.max_tenant_overlap > max_tenant_overlap {
            GateStatus::Failed(format!(
                "two tenants share {} of their {} cells, above the {} this topology permits; \
                 with {} possible shards the isolation shuffle sharding is supposed to buy is \
                 not being bought",
                metrics.max_tenant_overlap,
                cells_per_tenant,
                max_tenant_overlap,
                metrics.total_combinations,
            ))
        } else {
            GateStatus::Passed
        };

        ShuffleShardReport {
            status,
            metrics: Some(metrics),
        }
    }

    /// The certification pipeline's entry point. See the module docs: no
    /// mapping table is read, so there is no topology to judge.
    pub fn evaluate_without_topology_source(&self) -> ShuffleShardReport {
        Self::not_measured(MISSING_TOPOLOGY_SOURCE)
    }

    fn not_measured(reason: &str) -> ShuffleShardReport {
        ShuffleShardReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: reason.to_string(),
            },
            metrics: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(b: Vec<usize>) -> Vec<ShuffleShardAllocation> {
        vec![
            ShuffleShardAllocation {
                tenant_id: "tenant-a".to_string(),
                assigned_cells: vec![1, 2, 3, 4],
            },
            ShuffleShardAllocation {
                tenant_id: "tenant-b".to_string(),
                assigned_cells: b,
            },
        ]
    }

    #[test]
    fn an_absent_mapping_table_is_not_an_isolated_fleet() {
        let report = ShuffleShardSimulator::new().evaluate_without_topology_source();
        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(report.metrics.is_none());
    }

    #[test]
    fn a_topology_that_cannot_be_drawn_is_errored_before_any_ratio_is_published() {
        let sim = ShuffleShardSimulator::new();
        for (n, k) in [(2usize, 4usize), (8, 0)] {
            let report = sim.evaluate_topology(n, k, &pair(vec![1, 2]), 2);
            assert!(
                matches!(report.status, GateStatus::Errored(_)),
                "{k} cells per tenant out of {n} is not a topology"
            );
            assert!(report.metrics.is_none());
        }
        // The branch the guard above exists for: 1/C(n,k) is not a number when
        // no shard can be drawn, and a NaN in a published sentence is what the
        // coverage gate already cost this repository once.
        assert!(
            ShuffleShardMath::compute_metrics(2, 4, &[])
                .full_shard_overlap_ratio
                .is_nan()
        );
    }

    #[test]
    fn a_supplied_topology_is_judged_in_both_directions() {
        let sim = ShuffleShardSimulator::new();
        assert!(matches!(
            sim.evaluate_topology(8, 4, &pair(vec![5, 6, 7, 8]), 2)
                .status,
            GateStatus::Passed
        ));
        assert!(matches!(
            sim.evaluate_topology(8, 4, &pair(vec![2, 3, 4, 5]), 2)
                .status,
            GateStatus::Failed(_)
        ));
    }
}
