use crate::git_manager::diff_context::{FileChangeKind, diffs_by_path};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanManifestFinding {
    pub file_path: String,
    pub manifest_kind: String,
    pub reason: String,
}

pub struct OrphanSweeper;

impl Default for OrphanSweeper {
    fn default() -> Self {
        Self::new()
    }
}

impl OrphanSweeper {
    pub fn new() -> Self {
        Self
    }

    /// Whether a changed path is an ArgoCD desired-state manifest -- the scope
    /// this sweeper inspects.
    ///
    /// `pub` because the caller must distinguish "scanned and clean" from
    /// "nothing was in scope"; the predicate was inline and unreachable.
    ///
    /// It is a guess about filing convention, not about content: an ArgoCD
    /// `Application` or `ApplicationSet` is a Kubernetes resource identified by
    /// its `kind`, and nothing requires it to live at a path spelling either of
    /// these two fragments. A root app at `argocd/root.yaml` or a rendered
    /// Kustomize overlay is invisible here.
    pub fn is_gitops_manifest(file_path: &str) -> bool {
        file_path.contains("applicationset") || file_path.contains("application.yaml")
    }

    /// Findings over definite deletion observations, not proof of complete
    /// observation. The reconciler performs the fallible relevant-path preflight.
    pub fn scan_orphan_risk(
        &self,
        changed_files: &[String],
        diff_content: &str,
    ) -> Vec<OrphanManifestFinding> {
        let mut findings = Vec::new();

        // OPEN separate weakness: this finalizer exception still reads the
        // whole diff. Change-kind evidence does not establish its attribution.
        let finalizer_present = diff_content.contains("resources-finalizer");

        for file in diffs_by_path(diff_content)
            .into_iter()
            .filter(|file| changed_files.contains(&file.path))
        {
            if Self::is_gitops_manifest(&file.path)
                && file.change_kind() == Some(FileChangeKind::Deleted)
                && !finalizer_present
            {
                findings.push(OrphanManifestFinding {
                        file_path: file.path.clone(),
                        manifest_kind: "ApplicationSet".to_string(),
                        reason: "ArgoCD ApplicationSet deletion detected without explicit cascade-deletion finalizer protection (`resources-finalizer.argocd.argoproj.io`).".to_string(),
                    });
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_unsafe_applicationset_deletion() {
        let sweeper = OrphanSweeper::new();
        let changed = vec!["iac/apps/orphan-app-applicationset.yaml".to_string()];
        let diff = "diff --git a/iac/apps/orphan-app-applicationset.yaml b/iac/apps/orphan-app-applicationset.yaml\ndeleted file mode 100644\n--- a/iac/apps/orphan-app-applicationset.yaml\n+++ /dev/null";
        let findings = sweeper.scan_orphan_risk(&changed, diff);
        assert_eq!(findings.len(), 1);
    }
}
