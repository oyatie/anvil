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

    /// 100% Deterministic scan for deleted desired-state resources to ensure safe cascade deletion
    pub fn scan_orphan_risk(
        &self,
        changed_files: &[String],
        diff_content: &str,
    ) -> Vec<OrphanManifestFinding> {
        let mut findings = Vec::new();

        for file in changed_files {
            if file.contains("applicationset") || file.contains("application.yaml") {
                // If ApplicationSet is modified/deleted without specifying finalizers or cascade protection
                if diff_content.contains("deleted file")
                    && !diff_content.contains("resources-finalizer")
                {
                    findings.push(OrphanManifestFinding {
                        file_path: file.clone(),
                        manifest_kind: "ApplicationSet".to_string(),
                        reason: "ArgoCD ApplicationSet deletion detected without explicit cascade-deletion finalizer protection (`resources-finalizer.argocd.argoproj.io`).".to_string(),
                    });
                }
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
        let diff = "deleted file mode 100644\n--- a/iac/apps/orphan-app-applicationset.yaml\n+++ /dev/null";
        let findings = sweeper.scan_orphan_risk(&changed, diff);
        assert_eq!(findings.len(), 1);
    }
}
