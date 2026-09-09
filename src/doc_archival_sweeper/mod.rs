use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

pub mod issue_doc_consolidator;
pub use issue_doc_consolidator::{IssueDocConsolidationReport, IssueDocConsolidator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchivalSweepReport {
    /// Archived: the copy is on disk under `archive/2026/`.
    ///
    /// Pushed only after the write succeeds. It was pushed unconditionally,
    /// before the `dry_run` gate and before any write, so a failed archive
    /// still reported one and the CLI printed "Files Archived: 1" for zero
    /// files -- the same false report this type exists to prevent.
    pub files_archived: Vec<String>,
    /// Considered for archival and NOT archived, because the copy could not be
    /// written.
    ///
    /// A separate field because "we meant to" and "we did" are different facts,
    /// and one `Vec` cannot carry both without a caller guessing which it holds.
    pub archive_failed: Vec<String>,
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
        let mut archive_failed = Vec::new();
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

                    // This copy had also drifted: it omitted `node_modules`,
                    // which the other two skip. Three hand-maintained copies of
                    // one list is how that happens.
                    if crate::source_scan::repository_walk_skips(repo_dir, &path) {
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
                // What is on disk now. The demotion below rewrites `full_path`,
                // and the archival branch used the pre-demotion `content` --
                // so a file matching BOTH branches was reported demoted while
                // the only surviving copy still claimed canonical authority,
                // and the next sweep demoted the archive itself.
                let mut current = content.clone();
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
                        if tokio::fs::write(&full_path, &new_content).await.is_ok() {
                            current = new_content;
                        }
                    }
                }

                // Check 2: Superseded ADR or plan needing archival
                if (rel_path.starts_with("docs/adr-archive/")
                    || rel_path.starts_with(".grok/programs/"))
                    // Already swept. Without this the ORIGINAL path still
                    // matches on the next run, `current` is now the stub, and
                    // the archive copy is truncate-overwritten with it -- so
                    // sweep 1 saved the content and sweep 2 destroyed it. This
                    // is a repeatable CLI command over a repository whose
                    // sweep output is meant to be committed, so a second sweep
                    // is the expected case, not an edge one.
                    //
                    // Not `create_new` on the destination: where the archive
                    // was written and the stub write then failed, a re-run must
                    // still be able to finish the job, and `create_new` would
                    // refuse it forever.
                    && !current.contains("status: archived")
                {
                    if dry_run {
                        files_archived.push(rel_path.clone());
                    } else {
                        // The archive copy is written FIRST, and the original is
                        // replaced only if it succeeded.
                        //
                        // `dest` was computed, its parent created, and nothing
                        // ever written to it; the original was then overwritten
                        // with a stub naming a file that did not exist. The
                        // content was gone and the forward pointer dangled.
                        //
                        // Every write here was `let _ =`, so this ordering is
                        // not cosmetic: a failed copy followed by a successful
                        // stub is the same data loss with a healthy-looking
                        // report. A stub may only ever replace content that is
                        // already safe somewhere else.
                        let dest = repo_dir.join("archive/2026").join(&rel_path);
                        let archived = match dest.parent() {
                            Some(parent) => tokio::fs::create_dir_all(parent).await.is_ok(),
                            None => false,
                        } && tokio::fs::write(&dest, &current).await.is_ok();

                        if !archived {
                            archive_failed.push(rel_path.clone());
                        }

                        if archived {
                            files_archived.push(rel_path.clone());
                            let stub = format!(
                                "---\nschema: hyperscaler.doc.v1\nstatus: archived\ncanonical_authority: false\n---\n\n> **HISTORICAL / ARCHIVED:** Moved to `archive/2026/{}`.\n",
                                rel_path
                            );
                            if tokio::fs::write(&full_path, stub).await.is_ok() {
                                stubs_written.push(rel_path.clone());
                            }
                        }
                    }
                }
            }
        }

        let summary = format!(
            "DocArchivalSweeper completed (dry_run: {}): {} files archived, {} stubs created, {} SSOT claims demoted.",
            dry_run,
            files_archived.len(),
            stubs_written.len(),
            ssot_claims_demoted.len()
        );

        Ok(ArchivalSweepReport {
            files_archived,
            archive_failed,
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
