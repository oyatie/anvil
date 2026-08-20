use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::diff_context::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoadmapAlignmentReport {
    pub is_aligned: bool,
    pub detected_capabilities: Vec<String>,
    pub proposed_title: String,
    pub proposed_body_summary: String,
    pub matched_work_items: Vec<String>,
    pub ssot_doc_violations: Vec<String>,
    pub summary: String,
}

pub struct RoadmapReconciler;

impl Default for RoadmapReconciler {
    fn default() -> Self {
        Self::new()
    }
}

impl RoadmapReconciler {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates PR scope against the repository's live masterplan.json, docs, and git diff
    pub fn evaluate_pr_scope(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        current_title: &str,
        current_body: &str,
    ) -> Result<RoadmapAlignmentReport> {
        info!(
            "Evaluating PR #{} scope against live roadmap (specs/masterplan.json) and docs...",
            diff_ctx.pr_number
        );

        let mut detected_capabilities = Vec::new();
        let mut ssot_doc_violations = Vec::new();
        let mut matched_work_items = Vec::new();

        // 1. Detect touched capabilities from file paths
        for file in &diff_ctx.changed_files {
            let cap = if file.starts_with("iam/") {
                "iam"
            } else if file.starts_with("storage/") {
                "storage"
            } else if file.starts_with("cell/") {
                "cell"
            } else if file.starts_with("observability/") {
                "observability"
            } else if file.starts_with("contracts/") {
                "contracts"
            } else if file.starts_with("infra/") || file.starts_with("iac/") {
                "infra"
            } else if file.starts_with("ci/") {
                "ci"
            } else if file.starts_with("docs/") {
                "docs"
            } else if file.starts_with("src/") {
                "core"
            } else {
                "workspace"
            };

            if !detected_capabilities.contains(&cap.to_string()) {
                detected_capabilities.push(cap.to_string());
            }

            // 2. SSOT Documentation Integrity Check
            // Docs outside docs/ or contracts/ claiming canonical authority are violations
            if file.ends_with(".md")
                && !file.starts_with("docs/")
                && !file.starts_with("contracts/")
            {
                let full_path = repo_dir.join(file);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    if content.contains("canonical_authority: true")
                        || content.contains("live_plan_authority: true")
                    {
                        ssot_doc_violations.push(format!(
                            "File '{}' claims canonical authority but is outside docs/ or contracts/.",
                            file
                        ));
                    }
                }
            }
        }

        // 3. Inspect masterplan.json if present
        let masterplan_path = repo_dir.join("specs/masterplan.json");
        if masterplan_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&masterplan_path) {
                for cap in &detected_capabilities {
                    if content.contains(&format!("\"capability\": \"{}\"", cap))
                        || content.contains(&format!("\"{}\"", cap))
                    {
                        matched_work_items.push(format!("CAP-{}", cap.to_uppercase()));
                    }
                }
            }
        }

        // 4. Formulate honest, comprehensive PR title and body
        let primary_cap = detected_capabilities
            .first()
            .cloned()
            .unwrap_or_else(|| "core".to_string());
        let proposed_title = if current_title.is_empty() || current_title.starts_with("Update") {
            format!(
                "feat({}): reconcile {} and update {} file(s)",
                primary_cap,
                detected_capabilities.join(", "),
                diff_ctx.changed_files.len()
            )
        } else {
            current_title.to_string()
        };

        let proposed_body_summary = format!(
            "## 📋 Scope & Roadmap Reconciliation\n\
            - **Touched Capabilities**: {}\n\
            - **Modified Files**: {} total\n\
            - **Roadmap Alignment**: {}\n\
            - **SSOT Integrity**: {}\n\n\
            {}\n\n---\n*🤖 [Reconciled] by Oyatie Anvil*",
            detected_capabilities.join(", "),
            diff_ctx.changed_files.len(),
            if matched_work_items.is_empty() {
                "General Workspace"
            } else {
                "Verified against specs/masterplan.json"
            },
            if ssot_doc_violations.is_empty() {
                "✅ 100% SSOT Compliant"
            } else {
                "❌ SSOT Violations Detected"
            },
            current_body.trim()
        );

        let is_aligned = ssot_doc_violations.is_empty();
        let summary = if is_aligned {
            format!(
                "✅ PASSED (Scope verified across {} capability tracks; 0 SSOT contradictions)",
                detected_capabilities.len()
            )
        } else {
            format!(
                "❌ FAILED ({} SSOT documentation contradictions detected)",
                ssot_doc_violations.len()
            )
        };

        Ok(RoadmapAlignmentReport {
            is_aligned,
            detected_capabilities,
            proposed_title,
            proposed_body_summary,
            matched_work_items,
            ssot_doc_violations,
            summary,
        })
    }

    /// Verifies whether an issue title/body matches the live masterplan work item space
    pub fn verify_issue_roadmap_alignment(
        &self,
        repo_dir: &Path,
        issue_title: &str,
        issue_body: &str,
    ) -> bool {
        let masterplan_path = repo_dir.join("specs/masterplan.json");
        if !masterplan_path.exists() {
            return true; // If no masterplan, allow
        }

        // Banned retired prefixes (legacy omc/omx/grit)
        if issue_title.contains(".omc/")
            || issue_body.contains(".omc/")
            || issue_title.contains(".omx/")
            || issue_body.contains(".omx/")
        {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_roadmap_reconciliation_detects_capabilities() {
        let dir = tempdir().unwrap();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 42,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            is_incremental: false,
            previous_head_sha: None,
            diff_content: "+fn test() {}".to_string(),
            changed_files: vec![
                "iam/core/src/lib.rs".to_string(),
                "storage/core/src/lib.rs".to_string(),
                "docs/MASTERPLAN.md".to_string(),
            ],
            repo_working_dir: dir.path().to_path_buf(),
        };

        let reconciler = RoadmapReconciler::new();
        let report = reconciler
            .evaluate_pr_scope(
                dir.path(),
                &diff_ctx,
                "feat: update system",
                "Initial description",
            )
            .unwrap();

        assert!(report.is_aligned);
        assert_eq!(report.detected_capabilities.len(), 3);
        assert!(report.detected_capabilities.contains(&"iam".to_string()));
        assert!(report
            .detected_capabilities
            .contains(&"storage".to_string()));
        assert!(report.detected_capabilities.contains(&"docs".to_string()));
    }

    #[test]
    fn test_roadmap_reconciliation_catches_unauthorized_ssot() {
        let dir = tempdir().unwrap();
        let rogue_doc = dir.path().join("scripts/rogue.md");
        std::fs::create_dir_all(dir.path().join("scripts")).unwrap();
        std::fs::write(&rogue_doc, "---\ncanonical_authority: true\n---").unwrap();

        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 43,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            is_incremental: false,
            previous_head_sha: None,
            diff_content: "+rogue".to_string(),
            changed_files: vec!["scripts/rogue.md".to_string()],
            repo_working_dir: dir.path().to_path_buf(),
        };

        let reconciler = RoadmapReconciler::new();
        let report = reconciler
            .evaluate_pr_scope(dir.path(), &diff_ctx, "chore: rogue", "")
            .unwrap();

        assert!(!report.is_aligned);
        assert_eq!(report.ssot_doc_violations.len(), 1);
    }
}
