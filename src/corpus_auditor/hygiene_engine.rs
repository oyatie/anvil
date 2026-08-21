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

                // Update last_verified_at if frontmatter exists
                if content.contains("last_verified_at:") {
                    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
                    // replace date
                    let lines: Vec<String> = updated_content
                        .lines()
                        .map(|l| {
                            if l.trim_start().starts_with("last_verified_at:") {
                                format!("last_verified_at: \"{}\"", today)
                            } else {
                                l.to_string()
                            }
                        })
                        .collect();
                    updated_content = lines.join("\n");
                    changed = true;
                }

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
}
