//! CI wallclock and compute economics — the gate that timed nothing.
//!
//! # What was here
//!
//! `evaluate_ci_efficiency` constructed a `CiDurationSnapshot` from four
//! literals — one annotated in a source comment as comfortably inside the
//! per-push limit — and ran the regression budget over it. The PR duration and
//! the trunk baseline were both invented, so their ratio was fixed at compile
//! time: the budget could not be exceeded on any pull request. The summary then
//! published those same literals as a second count and a dollar figure, which a
//! reader has no way to distinguish from a reading off the CI provider (I2).
//!
//! # What is here now
//!
//! No snapshot is fabricated. Without the GitHub Actions workflow-run timing
//! API there is no duration and no billing data, so the gate reports
//! `GateStatus::NotMeasured` naming that source. It does not report `Failed`:
//! there is no evidence of a slow build, only an absence of timing.
//!
//! `RegressionBudgetEvaluator` and `CiCadenceClassifier` are retained and still
//! exported. Both are honest computations over a caller-supplied snapshot and
//! are the seam the timing API plugs into; only the caller that supplied itself
//! is deleted.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod cadence_classifier;
pub mod regression_budget;

pub use cadence_classifier::{CadenceRoutingFinding, CiCadenceClassifier};
pub use regression_budget::{
    CiDurationSnapshot, OptimizationSuggestion, RegressionBudgetEvaluator,
};

/// The data source that must exist before a wallclock can be reported.
const MISSING_TIMING_API: &str =
    "no GitHub Actions workflow-run timing API access is configured, so neither \
     this PR's CI duration nor its billable compute was read";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiWallclockReport {
    pub status: GateStatus,
    /// Whether the CI cost of this PR was established as acceptable. False
    /// while unmeasured: an unread duration cannot be asserted to be within
    /// budget.
    pub is_acceptable: bool,
    pub summary: String,
}

pub struct CiWallclockEconomicsRatchet;

impl Default for CiWallclockEconomicsRatchet {
    fn default() -> Self {
        Self::new()
    }
}

impl CiWallclockEconomicsRatchet {
    pub fn new() -> Self {
        Self
    }

    /// Reports CI wallclock and compute cost as unmeasured; see the module docs.
    pub fn evaluate_ci_efficiency(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<CiWallclockReport> {
        info!(
            "Running CiWallclockEconomicsRatchet (no timing API configured) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let summary = format!("➖ NOT MEASURED ({})", MISSING_TIMING_API);

        Ok(CiWallclockReport {
            status: GateStatus::NotMeasured {
                gate_id: "ci_wallclock_status".to_string(),
                reason: MISSING_TIMING_API.to_string(),
            },
            is_acceptable: false,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_timing_api_means_no_duration_and_no_cost() {
        // Replaces `test_ci_wallclock_ratchet_nominal`, which asserted
        // `rep.wallclock_seconds <= 300` against a duration the same function
        // had just written down.
        let ratchet = CiWallclockEconomicsRatchet::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn fast() {}".to_string(),
            changed_files: vec!["src/fast.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = ratchet
            .evaluate_ci_efficiency(Path::new("."), &diff_ctx)
            .expect("gate runs");
        assert_eq!(rep.status.unmeasured_gate_id(), Some("ci_wallclock_status"));
        assert!(!rep.summary.contains('$'), "{}", rep.summary);
    }
}
