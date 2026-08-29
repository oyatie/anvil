use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod orphan_sweeper;
pub use orphan_sweeper::{OrphanManifestFinding, OrphanSweeper};

use crate::pre_merge_guard::report::GateStatus;

const GATE_ID: &str = "gitops_drift_status";

const NO_MANIFEST_IN_SCOPE: &str = "no changed file matched the GitOps manifest marker set (`applicationset`, \
     `application.yaml`), so no desired-state deletion was scanned; an empty scope is not a \
     reconciled one";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitOpsDriftReport {
    pub status: GateStatus,
    pub is_safe: bool,
    pub orphan_findings: Vec<OrphanManifestFinding>,
    pub summary: String,
}

pub struct GitOpsDriftReconciler {
    sweeper: OrphanSweeper,
}

impl Default for GitOpsDriftReconciler {
    fn default() -> Self {
        Self::new()
    }
}

impl GitOpsDriftReconciler {
    pub fn new() -> Self {
        let sweeper = OrphanSweeper::new();
        Self { sweeper }
    }

    /// 100% Deterministic evaluation of ArgoCD / Flux ApplicationSet lifecycle and orphan drift
    pub fn evaluate_gitops_drift(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<GitOpsDriftReport> {
        info!(
            "Running GitOpsDriftReconciler (Deterministic Manifest Parity & Orphan Prevention) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let orphan_findings = self
            .sweeper
            .scan_orphan_risk(&diff_ctx.changed_files, &diff_ctx.diff_content);

        // Nothing in scope is not the same as nothing wrong. The marker set is
        // two path fragments, so a repository that files its manifests anywhere
        // else can never put one in scope, and a pass would certify a
        // reconciliation never performed.
        //
        // ArgoCD is not the precedent for this at its reporting layer -- the
        // opposite. `controller/state.go` starts at `SyncStatusCodeSynced` and
        // only downgrades inside the loop over target resources, so an
        // Application with no targets displays green; ArgoCD's own issue #26038
        // records the consequence, that "it is not possible to know if the
        // Application actually contained 0 resources ... or the cache was
        // unavailable". Where ArgoCD does refuse an empty scope is the *action*
        // layer: auto-sync declines when every managed resource would be pruned
        // ("auto-sync will wipe out all resources") unless `allowEmpty` is set
        // explicitly. That is the shape here. `NotMeasured` is
        // `is_acceptable()`, so the badge does not accuse the pull request of a
        // defect; `admission_refusal` withholds the merge.
        if !diff_ctx
            .changed_files
            .iter()
            .any(|f| OrphanSweeper::is_gitops_manifest(f))
        {
            return Ok(GitOpsDriftReport {
                status: GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason: NO_MANIFEST_IN_SCOPE.to_string(),
                },
                is_safe: false,
                orphan_findings,
                summary: NO_MANIFEST_IN_SCOPE.to_string(),
            });
        }

        let is_safe = orphan_findings.is_empty();

        let summary = if is_safe {
            "✅ PASSED (All GitOps ApplicationSets, Kustomizations, and Helm values maintain declarative integrity)".to_string()
        } else {
            format!(
                "⚠️ CAUTION ({} unmanaged/unsafe GitOps manifest deletion(s) detected)",
                orphan_findings.len()
            )
        };

        Ok(GitOpsDriftReport {
            status: if is_safe {
                GateStatus::Passed
            } else {
                GateStatus::Warning(summary.clone())
            },
            is_safe,
            orphan_findings,
            summary,
        })
    }
}

impl GitOpsDriftReport {
    /// Orphan manifests, as work items.
    ///
    /// Declared here rather than in `intake` so that `intake` stays a leaf.
    /// A finding belongs to the module that found it; the vocabulary it is
    /// expressed in must not import that module back.
    ///
    /// An orphan is mechanical to resolve -- the manifest is either adopted by
    /// a reconciler or removed -- so it is raised as such rather than as
    /// something awaiting a decision.
    pub fn work_items(&self, repo: &str) -> Vec<crate::intake::WorkItem> {
        use crate::intake::{Remedy, Source, WorkItem, sources::subject};
        self.orphan_findings
            .iter()
            .map(|f| WorkItem {
                source: Source::Drift,
                subject: subject(repo, &f.file_path),
                what: format!("orphan {} manifest: {}", f.manifest_kind, f.reason),
                consequence: "the manifest is applied by nothing, so what is \
                              declared and what runs have drifted and neither \
                              side reports it"
                    .to_string(),
                class: None,
                remedy: Remedy::Mechanical {
                    how: "adopt the manifest into a reconciler, or delete it".to_string(),
                },
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gitops_drift_reconciler_nominal() {
        let rec = GitOpsDriftReconciler::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ replicaCount: 3".to_string(),
            changed_files: vec!["infra/gitops/values.yaml".to_string()],
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("."),
                crate::git_manager::Uncloned::TestFixture,
            ),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = rec
            .evaluate_gitops_drift(Path::new("."), &diff_ctx)
            .unwrap();

        // This asserted `rep.is_safe` for a diff touching a Helm values file,
        // which is never in the manifest scope -- so it certified the vacuous
        // pass rather than testing the sweep. Out of scope is now unmeasured.
        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!rep.is_safe);
    }
}
