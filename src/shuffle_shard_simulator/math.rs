use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShuffleShardAllocation {
    pub tenant_id: String,
    pub assigned_cells: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadiusMetrics {
    pub total_cells: usize,
    pub cells_per_tenant: usize,
    pub total_combinations: usize,
    pub max_tenant_overlap: usize,
    /// Blast radius as the AWS Builders' Library and the Route 53 infima
    /// javadoc define it: the fraction of the tenant population that shares a
    /// given tenant's ENTIRE shuffle shard, which is 1/C(n,k).
    ///
    /// This field used to hold cells-per-tenant over total-cells. That number
    /// is real, but it is one tenant's infrastructure footprint, and it RISES
    /// as isolation improves — Route 53 gives every domain four of 2048 name
    /// servers, a footprint of 0.2% and a blast radius of one in 730 billion.
    /// Publishing the footprint as the blast radius inverted the sign of the
    /// claim.
    ///
    /// `f64::NAN` when no shard is combinatorially possible (`k > n`), which
    /// the caller is expected to reject before publishing anything.
    pub full_shard_overlap_ratio: f64,
}

pub struct ShuffleShardMath;

impl ShuffleShardMath {
    pub fn calculate_combinations(n: usize, k: usize) -> usize {
        if k > n {
            return 0;
        }
        if k == 0 || k == n {
            return 1;
        }
        let k = k.min(n - k);
        let mut c = 1;
        for i in 0..k {
            c = c * (n - i) / (i + 1);
        }
        c
    }

    /// Evaluates the maximum overlap between any two tenants in a set of shuffle shard assignments
    pub fn evaluate_overlap(allocations: &[ShuffleShardAllocation]) -> usize {
        let mut max_overlap = 0;
        for i in 0..allocations.len() {
            for j in (i + 1)..allocations.len() {
                let overlap = allocations[i]
                    .assigned_cells
                    .iter()
                    .filter(|c| allocations[j].assigned_cells.contains(c))
                    .count();
                if overlap > max_overlap {
                    max_overlap = overlap;
                }
            }
        }
        max_overlap
    }

    pub fn compute_metrics(
        total_cells: usize,
        cells_per_tenant: usize,
        allocations: &[ShuffleShardAllocation],
    ) -> BlastRadiusMetrics {
        let total_combinations = Self::calculate_combinations(total_cells, cells_per_tenant);
        let max_tenant_overlap = Self::evaluate_overlap(allocations);
        let full_shard_overlap_ratio = if total_combinations == 0 {
            f64::NAN
        } else {
            (total_combinations as f64).recip()
        };

        BlastRadiusMetrics {
            total_cells,
            cells_per_tenant,
            total_combinations,
            max_tenant_overlap,
            full_shard_overlap_ratio,
        }
    }

    /// Selects deterministic shuffle shard cells for a tenant using FNV-1a 64-bit hashing
    pub fn select_tenant_cells(
        tenant_id: &str,
        total_cells: usize,
        cells_per_tenant: usize,
    ) -> Vec<usize> {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in tenant_id.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        let bytes = hash.to_le_bytes();

        let mut available: Vec<usize> = (1..=total_cells).collect();
        let mut selected = Vec::new();
        for i in 0..cells_per_tenant.min(total_cells) {
            let idx = (bytes[i % bytes.len()] as usize + i) % available.len();
            selected.push(available.remove(idx));
        }
        selected.sort();
        selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shuffle_shard_combinatorics() {
        // N=8, K=4 => 70 combinations
        let combs = ShuffleShardMath::calculate_combinations(8, 4);
        assert_eq!(combs, 70);

        let t1 = ShuffleShardAllocation {
            tenant_id: "tenant-a".to_string(),
            assigned_cells: vec![1, 2, 3, 4],
        };
        let t2 = ShuffleShardAllocation {
            tenant_id: "tenant-b".to_string(),
            assigned_cells: vec![3, 4, 5, 6],
        };
        let overlap = ShuffleShardMath::evaluate_overlap(&[t1, t2]);
        assert_eq!(overlap, 2);
    }

    #[test]
    fn test_deterministic_subset_selection() {
        let cells1 = ShuffleShardMath::select_tenant_cells("tenant-alpha", 8, 2);
        let cells2 = ShuffleShardMath::select_tenant_cells("tenant-alpha", 8, 2);
        assert_eq!(cells1, cells2);
        assert_eq!(cells1.len(), 2);
        assert!(cells1[0] <= 8 && cells1[1] <= 8);

        let cells_beta = ShuffleShardMath::select_tenant_cells("tenant-beta", 8, 2);
        assert_eq!(cells_beta.len(), 2);
    }
}
