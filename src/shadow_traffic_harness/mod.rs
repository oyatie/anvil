//! Dark-traffic shadow replay — the harness that mirrored no traffic.
//!
//! # What was here
//!
//! `evaluate_shadow_verification` built a `ShadowTrafficMetrics` out of four
//! literals — a sample size, a payload parity, a status-code parity and a
//! latency delta — and handed it to a comparator whose thresholds those literals
//! cleared by construction. The published summary then named the sample size, so
//! a reader was told how many requests had been compared when none had been
//! sent. Naming a sample that was never drawn is the most persuasive form of the
//! defect: the number carries its own apparent provenance (I2).
//!
//! # What is here now
//!
//! No metrics are fabricated. Without traffic mirroring infrastructure and a
//! replay target there is nothing to compare, so the gate reports
//! `GateStatus::NotMeasured` naming both missing pieces. It does not report a
//! parity failure: no response diverged, because no response was ever produced.
//!
//! `TrafficMirrorComparator` is retained and still exported. It is an honest
//! computation over caller-supplied metrics and is the seam a real mirror plugs
//! into; only the caller that supplied itself is deleted.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod traffic_mirror;
pub use traffic_mirror::{ShadowTrafficMetrics, TrafficMirrorComparator};

/// The infrastructure that must exist before response parity can be compared.
const MISSING_TRAFFIC_MIRROR: &str = "no traffic mirror and no replay target are configured, so no production \
     requests were sampled and no responses were compared";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowTrafficReport {
    pub status: GateStatus,
    /// Whether shadow parity was established. False while unmeasured: an
    /// un-mirrored deployment cannot be asserted to behave identically.
    pub is_verified: bool,
    pub summary: String,
}

pub struct ShadowTrafficHarness;

impl Default for ShadowTrafficHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl ShadowTrafficHarness {
    pub fn new() -> Self {
        Self
    }

    /// Reports shadow replay parity as unmeasured; see the module docs.
    pub fn evaluate_shadow_verification(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<ShadowTrafficReport> {
        info!(
            "Running ShadowTrafficHarness (no mirror or replay target configured) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let summary = format!("➖ NOT MEASURED ({})", MISSING_TRAFFIC_MIRROR);

        Ok(ShadowTrafficReport {
            status: GateStatus::NotMeasured {
                gate_id: "shadow_traffic_status".to_string(),
                reason: MISSING_TRAFFIC_MIRROR.to_string(),
            },
            is_verified: false,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_mirror_means_no_sample_size_and_no_parity() {
        // Replaces `test_shadow_harness_nominal`, which asserted `rep.is_verified`
        // over metrics the same function had just written down.
        let harness = ShadowTrafficHarness::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn handler() {}".to_string(),
            changed_files: vec!["src/handler.rs".to_string()],
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("."),
                crate::git_manager::Uncloned::TestFixture,
            ),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = harness
            .evaluate_shadow_verification(Path::new("."), &diff_ctx)
            .expect("gate runs");
        assert_eq!(
            rep.status.unmeasured_gate_id(),
            Some("shadow_traffic_status")
        );
        assert!(
            !rep.summary.to_lowercase().contains("verified"),
            "{}",
            rep.summary
        );
    }
}
