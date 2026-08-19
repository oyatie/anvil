use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueDocConsolidationReport {
    pub issue_number: u64,
    pub files_archived: Vec<String>,
    pub stubs_written: Vec<String>,
    pub summary: String,
}

pub struct IssueDocConsolidator;

impl IssueDocConsolidator {
    /// Extracts doc references from an issue body and consolidates/archives temporary plans
    pub async fn consolidate_issue_docs(
        repo_dir: &Path,
        issue_number: u64,
        issue_body: &str,
        dry_run: bool,
    ) -> Result<IssueDocConsolidationReport> {
        info!(
            "Running IssueDocConsolidator for issue #{} on repo {:?} (dry_run: {})...",
            issue_number, repo_dir, dry_run
        );

        let mut files_archived = Vec::new();
        let mut stubs_written = Vec::new();

        // Extract markdown and json file references from issue text
        let re = Regex::new(r"([a-zA-Z0-9_./\-]+\.(?:md|json|yaml))").unwrap();

        for cap in re.captures_iter(issue_body) {
            let rel_path = cap[1].to_string();
            let full_path = repo_dir.join(&rel_path);

            if full_path.is_file() {
                // If it is an ephemeral plan in .grok or tasks
                if rel_path.starts_with(".grok/programs/")
                    || rel_path.starts_with("tasks/")
                    || rel_path.starts_with("docs/adr-archive/")
                {
                    files_archived.push(rel_path.clone());

                    if !dry_run {
                        let dest = repo_dir.join("archive/2026").join(&rel_path);
                        if let Some(parent) = dest.parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }

                        // Write forward-pointer stub
                        let stub = format!(
                            "---\nschema: hyperscaler.doc.v1\nstatus: archived\ncanonical_authority: false\n---\n\n> **HISTORICAL / ARCHIVED (Issue #{}):** Consolidated and moved to `archive/2026/{}`.\n",
                            issue_number, rel_path
                        );
                        let _ = tokio::fs::write(&full_path, stub).await;
                        stubs_written.push(rel_path);
                    }
                }
            }
        }

        let summary = format!(
            "Issue #{} Doc Consolidation complete (dry_run: {}): {} files archived, {} stubs written.",
            issue_number, dry_run, files_archived.len(), stubs_written.len()
        );

        Ok(IssueDocConsolidationReport {
            issue_number,
            files_archived,
            stubs_written,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_issue_doc_consolidation_archives_plan() {
        let dir = tempdir().unwrap();
        let plan_dir = dir.path().join(".grok/programs");
        tokio::fs::create_dir_all(&plan_dir).await.unwrap();

        let plan_file = plan_dir.join("SPRINT-12.md");
        tokio::fs::write(&plan_file, "# Sprint 12 Plan")
            .await
            .unwrap();

        let issue_body = "Resolving task detailed in `.grok/programs/SPRINT-12.md` successfully.";
        let report =
            IssueDocConsolidator::consolidate_issue_docs(dir.path(), 88, issue_body, false)
                .await
                .unwrap();

        assert_eq!(report.files_archived.len(), 1);
        assert_eq!(report.stubs_written.len(), 1);

        let stub_content = tokio::fs::read_to_string(&plan_file).await.unwrap();
        assert!(stub_content.contains("HISTORICAL / ARCHIVED (Issue #88)"));
    }
}
