use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

pub mod issue_doc_consolidator;
pub use issue_doc_consolidator::{IssueDocConsolidationReport, IssueDocConsolidator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivalSweepReport {
    pub files_archived: Vec<String>,
    pub stubs_written: Vec<String>,
    pub ssot_claims_demoted: Vec<String>,
    pub is_dry_run: bool,
    pub summary: String,
}

pub struct DocArchivalSweeper;

impl DocArchivalSweeper {
    /// Scans monorepo for stale sprint plans, superseded ADRs, and unauthorized SSOT claims
    pub async fn sweep_repository(repo_dir: &Path, dry_run: bool) -> Result<ArchivalSweepReport> {
        info!(
            "Running DocArchivalSweeper on repo at {:?} (dry_run: {})...",
            repo_dir, dry_run
        );

        let mut files_archived = Vec::new();
        let mut stubs_written = Vec::new();
        let mut ssot_claims_demoted = Vec::new();

        // 1. Scan for files outside docs/ and contracts/ declaring canonical authority
        let mut files_to_scan = Vec::new();
        let mut stack = vec![repo_dir.to_path_buf()];

        while let Some(dir) = stack.pop() {
            if let Ok(mut entries) = tokio::fs::read_dir(&dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    let rel = path
                        .strip_prefix(repo_dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    // Skip ignored dirs
                    if rel.starts_with(".git")
                        || rel.starts_with("target")
                        || rel.starts_with("buck-out")
                    {
                        continue;
                    }

                    if path.is_dir() {
                        stack.push(path);
                    } else if path.is_file()
                        && (rel.ends_with(".md") || rel.ends_with(".yaml") || rel.ends_with(".yml"))
                    {
                        files_to_scan.push((path, rel));
                    }
                }
            }
        }

        for (full_path, rel_path) in files_to_scan {
            if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                // Check 1: Unauthorized SSOT claim
                let is_canonical_dir =
                    rel_path.starts_with("docs/") || rel_path.starts_with("contracts/");
                if !is_canonical_dir
                    && (content.contains("canonical_authority: true")
                        || (content.contains("source of truth") && content.contains("canonical")))
                {
                    ssot_claims_demoted.push(rel_path.clone());
                    if !dry_run {
                        let new_content = content
                            .replace("canonical_authority: true", "canonical_authority: false");
                        let _ = tokio::fs::write(&full_path, new_content).await;
                    }
                }

                // Check 2: Superseded ADR or plan needing archival
                if rel_path.starts_with("docs/adr-archive/")
                    || rel_path.starts_with(".grok/programs/")
                {
                    files_archived.push(rel_path.clone());
                    if !dry_run {
                        let dest = repo_dir.join("archive/2026").join(&rel_path);
                        if let Some(parent) = dest.parent() {
                            let _ = tokio::fs::create_dir_all(parent).await;
                        }
                        // Write forward-pointer stub
                        let stub = format!(
                            "---\nschema: hyperscaler.doc.v1\nstatus: archived\ncanonical_authority: false\n---\n\n> **HISTORICAL / ARCHIVED:** Moved to `archive/2026/{}`.\n",
                            rel_path
                        );
                        let _ = tokio::fs::write(&full_path, stub).await;
                        stubs_written.push(rel_path.clone());
                    }
                }
            }
        }

        let summary = format!(
            "DocArchivalSweeper completed (dry_run: {}): {} files archived, {} stubs created, {} SSOT claims demoted.",
            dry_run, files_archived.len(), stubs_written.len(), ssot_claims_demoted.len()
        );

        Ok(ArchivalSweepReport {
            files_archived,
            stubs_written,
            ssot_claims_demoted,
            is_dry_run: dry_run,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_doc_archival_sweeper_demotes_ssot_and_archives() {
        let dir = tempdir().unwrap();
        let tenancy_dir = dir.path().join("tenancy");
        tokio::fs::create_dir_all(&tenancy_dir).await.unwrap();

        let test_file = tenancy_dir.join("policy.md");
        tokio::fs::write(
            &test_file,
            "---\ncanonical_authority: true\n---\n# Tenancy Policy",
        )
        .await
        .unwrap();

        let report = DocArchivalSweeper::sweep_repository(dir.path(), false)
            .await
            .unwrap();
        assert_eq!(report.ssot_claims_demoted.len(), 1);
        assert!(report.ssot_claims_demoted[0].contains("tenancy/policy.md"));

        let updated_content = tokio::fs::read_to_string(&test_file).await.unwrap();
        assert!(updated_content.contains("canonical_authority: false"));
    }
}
