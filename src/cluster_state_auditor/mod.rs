//! Live cluster readback against declared desired state — the readback that
//! never happened.
//!
//! # What was here
//!
//! `evaluate_cluster_parity` assigned two local variables the *same* string
//! literal and passed them to the differ as "live" and "Git". A comparison of a
//! value against itself has one possible outcome, so the gate published
//! `✅ PASSED (Live cluster state is 100% synchronized with Git declarative
//! desired-state)` on every pull request without a cluster existing anywhere in
//! the process. This is the purest form of the defect in this lane: not a
//! threshold that could not be crossed, but two operands that could not differ.
//!
//! # What is here now
//!
//! No manifests are invented. Without Kubernetes API or ArgoCD access there is
//! no live state to read, so the gate reports `GateStatus::NotMeasured` naming
//! that missing access, publishes no claim of synchronization, and alleges no
//! drift — there is no evidence of either.
//!
//! `ClusterDiffEvaluator` was rewritten in the same change to compare whatever
//! it is given, rather than one hardcoded pair, so that it is a seam a real
//! readback can be plugged into instead of a second constant.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod diff_evaluator;
pub use diff_evaluator::{ClusterDiffEvaluator, ClusterDriftFinding};

/// The access that must exist before live state can be read back.
const MISSING_CLUSTER_ACCESS: &str =
    "no Kubernetes API or ArgoCD cluster access is configured, so no live state \
     was read back and no comparison against Git was performed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterAuditReport {
    pub status: GateStatus,
    /// Whether live state was established to match Git. False while unmeasured:
    /// an uncontacted cluster cannot be asserted to be in sync.
    pub is_synchronized: bool,
    /// Empty while unmeasured. Drift that was never observed is not alleged.
    pub drift_findings: Vec<ClusterDriftFinding>,
    pub summary: String,
}

pub struct ClusterStateAuditor;

impl Default for ClusterStateAuditor {
    fn default() -> Self {
        Self::new()
    }
}

impl ClusterStateAuditor {
    pub fn new() -> Self {
        Self
    }

    /// Reports live-versus-declared parity as unmeasured; see the module docs.
    pub fn evaluate_cluster_parity(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<ClusterAuditReport> {
        info!(
            "Running ClusterStateAuditor (no cluster access configured) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let summary = format!("➖ NOT MEASURED ({})", MISSING_CLUSTER_ACCESS);

        Ok(ClusterAuditReport {
            status: GateStatus::NotMeasured {
                gate_id: "cluster_audit_status".to_string(),
                reason: MISSING_CLUSTER_ACCESS.to_string(),
            },
            is_synchronized: false,
            drift_findings: Vec::new(),
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_cluster_access_means_no_parity_claim_and_no_drift_allegation() {
        // Replaces `test_cluster_auditor_nominal`, which asserted
        // `rep.is_synchronized` after comparing a literal against itself.
        let auditor = ClusterStateAuditor::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ config: true".to_string(),
            changed_files: vec!["infra/config.yaml".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = auditor
            .evaluate_cluster_parity(Path::new("."), &diff_ctx)
            .expect("gate runs");
        assert_eq!(
            rep.status.unmeasured_gate_id(),
            Some("cluster_audit_status")
        );
        assert!(rep.drift_findings.is_empty());
        assert!(
            !rep.summary.to_lowercase().contains("synchronized"),
            "{}",
            rep.summary
        );
    }
}
