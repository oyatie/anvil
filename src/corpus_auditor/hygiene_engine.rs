use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use super::freshness_ledger::FreshnessLedger;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HygieneBatchReport {
    pub batch_id: String,
    pub files_modified: Vec<String>,
    pub is_dry_run: bool,
    pub summary: String,
}

pub struct ContinuousHygieneEngine;

impl ContinuousHygieneEngine {
    /// Generates an autonomous maintenance batch PR to keep dark code fresh and clean
    pub fn generate_maintenance_batch(
        repo_dir: &Path,
        batch_size: usize,
        dry_run: bool,
    ) -> Result<HygieneBatchReport> {
        let batch_id = format!(
            "anvil-hygiene-{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        );
        info!(
            "Generating Continuous Hygiene batch '{}' on repo {:?} (batch_size: {}, dry_run: {})...",
            batch_id, repo_dir, batch_size, dry_run
        );

        let freshness = FreshnessLedger::scan_repository(repo_dir, 180);
        let mut files_modified = Vec::new();

        // Select top candidates from dormant files or unauthorized SSOT files
        for record in freshness.dormant_files.iter().take(batch_size) {
            let full_path = repo_dir.join(&record.file_path);
            if full_path.is_file()
                && let Ok(content) = std::fs::read_to_string(&full_path)
            {
                let mut updated_content = content.clone();
                let mut changed = false;

                // No `last_verified_at` restamp here. These files were selected
                // BECAUSE they are dormant, and stamping today's date onto one
                // verifies nothing -- it only stops the ledger reporting the
                // staleness. Freshness is a claim; a batch that cannot check it
                // must not publish it.
                // Demote unauthorized SSOT if outside docs/
                if !record.file_path.starts_with("docs/")
                    && !record.file_path.starts_with("contracts/")
                    && updated_content.contains("canonical_authority: true")
                {
                    updated_content = updated_content
                        .replace("canonical_authority: true", "canonical_authority: false");
                    changed = true;
                }

                if changed {
                    files_modified.push(record.file_path.clone());
                    if !dry_run {
                        let _ = std::fs::write(&full_path, updated_content);
                    }
                }
            }
        }

        let summary = format!(
            "Continuous Hygiene Batch '{}' generated (dry_run: {}): {} files refreshed/healed.",
            batch_id,
            dry_run,
            files_modified.len()
        );

        Ok(HygieneBatchReport {
            batch_id,
            files_modified,
            is_dry_run: dry_run,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hygiene_batch_generation() {
        let dir = tempdir().unwrap();
        let doc = dir.path().join("docs/runbook.md");
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(
            &doc,
            "---\nlast_verified_at: \"2025-01-01\"\n---\n# Runbook",
        )
        .unwrap();

        let report =
            ContinuousHygieneEngine::generate_maintenance_batch(dir.path(), 5, false).unwrap();
        assert!(report.summary.contains("Continuous Hygiene Batch"));
    }

    /// The batch picks files BECAUSE they are dormant. Restamping the date it
    /// selected them on would make every dormant document look verified and
    /// leave nothing for a staleness check to find.
    ///
    /// The fixture is backdated with `touch`, because dormancy is decided by
    /// mtime: a file written during the test is fresh, so it is never selected
    /// and the assertion would hold no matter what the batch does.
    #[test]
    fn a_dormant_document_is_not_restamped_as_freshly_verified() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        let page = dir.path().join("docs/runbook.md");
        std::fs::write(
            &page,
            "---\nlast_verified_at: \"2025-01-01\"\n---\n# Runbook\n",
        )
        .unwrap();
        assert!(
            std::process::Command::new("touch")
                .args(["-t", "202401010000"])
                .arg(&page)
                .status()
                .expect("touch")
                .success(),
            "could not backdate the fixture, so it would not be dormant"
        );

        ContinuousHygieneEngine::generate_maintenance_batch(dir.path(), 5, false).unwrap();

        assert!(
            std::fs::read_to_string(&page)
                .unwrap()
                .contains("2025-01-01"),
            "the dormant date was rewritten, so the document now claims a verification \
             that nobody performed"
        );
    }
}
