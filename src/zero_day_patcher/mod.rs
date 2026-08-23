use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod advisory_listener;
pub use advisory_listener::{AdvisoryListener, SecurityAdvisory};

const GATE_ID: &str = "zero_day_status";

/// Why this gate reports nothing.
///
/// Two independent reasons, both structural:
///
///   - **No advisory feed.** The evaluation opened with
///     `let active_advisories = vec![];` and handed that empty slice to
///     `reconcile_advisories`, whose loop then ran zero times. `is_clean` was
///     `true` on every pull request that has ever been certified. The matcher's
///     parameter is named `lockfile_content` and the caller passed
///     `diff_ctx.diff_content`, so no lockfile was ever read either.
///   - **No patch synthesis.** Nothing in this module writes a patch, edits a
///     manifest, or opens a pull request. Dependabot and Renovate do not
///     synthesise a fix either -- they substitute an already-released version --
///     but they at least need write access, a bot identity and a branch. None of
///     that exists here.
///
/// The detection half is now performed for real by `supply_chain_status`, which
/// resolves `Cargo.lock` and queries OSV.dev. What remains uniquely claimed
/// here is the synthesis, and that has no implementation.
const NO_PATCH_SYNTHESIS: &str = "no advisory feed is read and no patch is ever synthesised: the evaluation matched an \
     empty advisory list against the pull request diff, and nothing in this gate writes a \
     patch, edits a manifest or opens a pull request. Advisory detection against the locked \
     dependency graph is performed by supply_chain_status";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroDayReport {
    pub status: GateStatus,
    pub summary: String,
}

#[derive(Default)]
pub struct ZeroDayAutoPatcher;

impl ZeroDayAutoPatcher {
    pub fn new() -> Self {
        Self
    }

    /// Reports that no zero-day patch was synthesised, because none can be.
    pub fn evaluate_zero_day_patches(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<ZeroDayReport> {
        info!(
            "ZeroDayAutoPatcher has no advisory feed and no patch writer; reporting \
             NotMeasured for {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        Ok(ZeroDayReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: NO_PATCH_SYNTHESIS.to_string(),
            },
            summary: NO_PATCH_SYNTHESIS.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The diff that used to be certified clean: `vec![]` made every input
    /// clean, so no diff exists that could have failed this gate.
    #[test]
    fn no_diff_can_produce_a_pass() {
        let diff_ctx = PrDiffContext {
            repo: "oyatie/anvil".to_string(),
            pr_number: 100,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ time = \"0.1.44\"".to_string(),
            changed_files: vec!["Cargo.toml".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = ZeroDayAutoPatcher::new()
            .evaluate_zero_day_patches(Path::new("."), &diff_ctx)
            .expect("the gate answers");

        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
    }
}
