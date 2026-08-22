use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod allocation_scanner;
pub use allocation_scanner::{AllocationScanner, HeapAllocationFinding};

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "finops_status";

const NO_HOTPATH_IN_SCOPE: &str = "no changed file matched the hotpath marker set, so no allocation \
     was scanned; an empty scope is not a clean scan";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinOpsReport {
    pub status: GateStatus,
    pub is_cost_optimal: bool,
    pub findings: Vec<HeapAllocationFinding>,
    pub summary: String,
}

pub struct FinOpsUnitCostRatchet {
    scanner: AllocationScanner,
}

impl Default for FinOpsUnitCostRatchet {
    fn default() -> Self {
        Self::new()
    }
}

impl FinOpsUnitCostRatchet {
    pub fn new() -> Self {
        let scanner = AllocationScanner::new();
        Self { scanner }
    }

    /// Evaluates PR diffs for zero-copy semantics and unnecessary heap allocations in hotpaths
    pub fn evaluate_unit_cost(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<FinOpsReport> {
        info!(
            "Running FinOpsUnitCostRatchet (Zero-Copy Hotpaths & Cost-Per-Outcome Ratchet) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings = Vec::new();
        let mut scanned_a_hotpath = false;

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            let lines: Vec<&str> = file_diff.lines().collect();
            let mut current_file = "unknown.rs".to_string();
            if let Some(first_line) = lines.first()
                && let Some(path) = first_line.split_whitespace().last()
            {
                current_file = path.trim_start_matches("b/").to_string();
            }

            if AllocationScanner::is_hotpath(&current_file) {
                scanned_a_hotpath = true;
            }

            let file_findings = self
                .scanner
                .scan_hotpath_allocations(&current_file, file_diff);
            findings.extend(file_findings);
        }

        // Nothing in scope is not the same as nothing wrong. The marker set is
        // a fixed list of path fragments, and a repository that uses none of
        // them can never put a file in scope -- so a pass here would certify a
        // hotpath budget that was never examined.
        if !scanned_a_hotpath {
            return Ok(FinOpsReport {
                status: GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason: NO_HOTPATH_IN_SCOPE.to_string(),
                },
                is_cost_optimal: false,
                findings,
                summary: NO_HOTPATH_IN_SCOPE.to_string(),
            });
        }

        let is_cost_optimal = findings.is_empty();
        let summary = if is_cost_optimal {
            "✅ PASSED (Zero-copy semantics and zero unbudgeted heap allocations in hotpaths)"
                .to_string()
        } else {
            format!(
                "⚠️ CAUTION ({} avoidable hotpath heap allocation(s) detected)",
                findings.len()
            )
        };

        Ok(FinOpsReport {
            status: if is_cost_optimal {
                GateStatus::Passed
            } else {
                GateStatus::Warning(summary.clone())
            },
            is_cost_optimal,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finops_ratchet_nominal() {
        let ratchet = FinOpsUnitCostRatchet::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ pub fn sum(a: &[u8]) -> usize { a.len() }".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = ratchet
            .evaluate_unit_cost(Path::new("."), &diff_ctx)
            .unwrap();

        // This asserted `rep.is_cost_optimal` for a diff touching src/lib.rs,
        // which is never in the hotpath scope -- so it certified the vacuous
        // pass rather than testing the scan. Out of scope is now unmeasured.
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!rep.is_cost_optimal);
    }
}
