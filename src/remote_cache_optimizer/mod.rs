//! Remote build-cache alignment — the gate that had no cache to look at.
//!
//! # What was here
//!
//! `evaluate_cache_alignment` built a `CacheHitMetrics` out of four literals and
//! handed it to a ratchet whose threshold those literals cleared by construction.
//! It then computed a cache key from a literal lockfile name chosen so that no
//! file needed to exist, and published that key on the pull request as though a
//! real lockfile had been hashed.
//!
//! Two fabrications, one shape: an identifier and a rate, both invented, both
//! presented as measurements (I2).
//!
//! # What is here now
//!
//! Nothing is invented and nothing is guessed. Without sccache or Buck2 CAS
//! statistics there is no hit rate and no lockfile digest, so the gate reports
//! `GateStatus::NotMeasured` naming that missing source.
//!
//! `CacheHitRateRatchet` and `CacheKeyGenerator` are deliberately retained and
//! still exported: both are honest pure functions over caller-supplied input,
//! and they are the seam a real CAS statistics client plugs into. What is
//! deleted is the caller that supplied itself.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod bitrot_scrubber;
pub mod cache_hit_ratchet;
pub mod cache_keys;

pub use bitrot_scrubber::{CasBitRotScrubber, CasScrubReport};
pub use cache_hit_ratchet::{CacheHitMetrics, CacheHitRateRatchet};
pub use cache_keys::CacheKeyGenerator;

/// The data source that must exist before a hit rate can be reported.
const MISSING_CAS_STATISTICS: &str =
    "no sccache or Buck2 CAS statistics endpoint is configured, so no cache hit \
     rate was read and no lockfile was hashed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheReport {
    pub status: GateStatus,
    /// Whether cache alignment was established. False while unmeasured: an
    /// unread cache cannot be asserted to be aligned.
    pub is_cache_aligned: bool,
    pub summary: String,
}

pub struct RemoteCacheOptimizer;

impl Default for RemoteCacheOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

impl RemoteCacheOptimizer {
    pub fn new() -> Self {
        Self
    }

    /// Reports remote cache alignment as unmeasured; see the module docs.
    pub fn evaluate_cache_alignment(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<CacheReport> {
        info!(
            "Running RemoteCacheOptimizer (no CAS statistics source configured) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let summary = format!("➖ NOT MEASURED ({})", MISSING_CAS_STATISTICS);

        Ok(CacheReport {
            status: GateStatus::NotMeasured {
                gate_id: "remote_cache_status".to_string(),
                reason: MISSING_CAS_STATISTICS.to_string(),
            },
            is_cache_aligned: false,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_cas_source_means_no_hit_rate_and_no_cache_key() {
        // Replaces `test_cache_optimizer_nominal`, which asserted
        // `rep.hit_rate_pct >= 90.0` against a rate the same function had just
        // written down.
        let opt = RemoteCacheOptimizer::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn x() {}".to_string(),
            changed_files: vec!["src/x.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = opt
            .evaluate_cache_alignment(Path::new("."), &diff_ctx)
            .expect("gate runs");
        assert_eq!(rep.status.unmeasured_gate_id(), Some("remote_cache_status"));
        assert!(!rep.summary.contains("sccache-v2-"), "{}", rep.summary);
    }
}
