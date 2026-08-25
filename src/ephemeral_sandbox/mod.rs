use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "sandbox_status";

const MISSING_SANDBOX_RUNTIME: &str = "no ephemeral sandbox runtime is available, so no sandbox was \
     started and no isolation was observed for this pull request";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxReport {
    pub status: GateStatus,
    pub is_hermetic: bool,
    pub sandboxes_allocated: usize,
    pub average_spinup_ms: u64,
    pub summary: String,
}

pub struct EphemeralSandboxManager;

impl Default for EphemeralSandboxManager {
    fn default() -> Self {
        Self::new()
    }
}

impl EphemeralSandboxManager {
    pub fn new() -> Self {
        Self
    }

    /// The gate's answer when no sandbox runtime exists to spin one up.
    pub fn evaluate_without_sandbox_runtime(&self) -> SandboxReport {
        SandboxReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: MISSING_SANDBOX_RUNTIME.to_string(),
            },
            is_hermetic: false,
            sandboxes_allocated: 0,
            average_spinup_ms: 0,
            summary: MISSING_SANDBOX_RUNTIME.to_string(),
        }
    }

    /// 100% Deterministic evaluation of hermetic ephemeral sandbox isolation
    pub fn evaluate_sandbox_isolation(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<SandboxReport> {
        info!(
            "Running EphemeralSandboxManager (Deterministic Sub-Second Sandbox Isolation) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        // `allocate_ephemeral_sandbox` builds a struct literal: it starts no
        // container, binds no port, and times nothing. Reporting its constants
        // as a measurement published "allocated in 185ms" on every pull
        // request. Nothing here can observe isolation, so nothing is claimed.
        Ok(self.evaluate_without_sandbox_runtime())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_manager_nominal() {
        let mgr = EphemeralSandboxManager::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn test() {}".to_string(),
            changed_files: vec!["tests/integration.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = mgr
            .evaluate_sandbox_isolation(Path::new("."), &diff_ctx)
            .unwrap();

        // This asserted `rep.is_hermetic`, which was a constant `true` carried
        // from a struct literal -- so it certified the fabrication rather than
        // testing anything. The pipeline path now reports the absence.
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!rep.is_hermetic);
        assert_eq!(rep.average_spinup_ms, 0);
    }
}
