use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use super::freshness_ledger::{FreshnessLedger, FreshnessLedgerReport};
use crate::doc_guard::frontmatter::FrontmatterValidator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusAuditReport {
    pub total_files: usize,
    pub freshness_ratio: f64,
    pub dormant_files_count: usize,
    pub stale_adrs_count: usize,
    pub unauthorized_ssot_claims: Vec<String>,
    pub frontmatter_violations: Vec<String>,
    pub summary: String,
}

pub struct CorpusAuditor;

impl CorpusAuditor {
    /// Audits 100% of files in the repository to eliminate dark code and stale documentation
    pub fn audit_repository(repo_dir: &Path, stale_days: u64) -> Result<CorpusAuditReport> {
        info!(
            "Running full-corpus audit on repository at {:?} (threshold: {} days)...",
            repo_dir, stale_days
        );

        // 1. Scan Freshness Ledger
        let freshness_report: FreshnessLedgerReport =
            FreshnessLedger::scan_repository(repo_dir, stale_days);

        let mut unauthorized_ssot_claims = Vec::new();
        let mut frontmatter_violations = Vec::new();
        let mut stale_adrs_count = 0;

        // 2. Scan whole repository for SSOT and frontmatter compliance
        let mut stack = vec![repo_dir.to_path_buf()];

        while let Some(dir) = stack.pop() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let rel = path
                        .strip_prefix(repo_dir)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .to_string();

                    if rel.starts_with(".git")
                        || rel.starts_with("target")
                        || rel.starts_with("buck-out")
                        || rel.starts_with("node_modules")
                    {
                        continue;
                    }

                    if path.is_dir() {
                        stack.push(path);
                    } else if path.is_file()
                        && (rel.ends_with(".md") || rel.ends_with(".yaml") || rel.ends_with(".yml"))
                        && let Ok(content) = std::fs::read_to_string(&path)
                    {
                        // Check SSOT claim location
                        let is_canonical_dir =
                            rel.starts_with("docs/") || rel.starts_with("contracts/");
                        if !is_canonical_dir
                            && (content.contains("canonical_authority: true")
                                || (content.contains("source of truth")
                                    && content.contains("canonical")))
                        {
                            unauthorized_ssot_claims.push(rel.clone());
                        }

                        // Check frontmatter validity
                        if let Err(err) =
                            FrontmatterValidator::validate_doc_frontmatter(&rel, &content, repo_dir)
                        {
                            frontmatter_violations.push(err);
                        }

                        if rel.starts_with("docs/adr-archive/") {
                            stale_adrs_count += 1;
                        }
                    }
                }
            }
        }

        let summary = format!(
            "Corpus Audit Complete: {} total files, {:.1}% fresh, {} dormant files (>{}d), {} unauthorized SSOT claims, {} frontmatter violations.",
            freshness_report.total_files,
            freshness_report.freshness_ratio * 100.0,
            freshness_report.dormant_files_count,
            stale_days,
            unauthorized_ssot_claims.len(),
            frontmatter_violations.len()
        );

        Ok(CorpusAuditReport {
            total_files: freshness_report.total_files,
            freshness_ratio: freshness_report.freshness_ratio,
            dormant_files_count: freshness_report.dormant_files_count,
            stale_adrs_count,
            unauthorized_ssot_claims,
            frontmatter_violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_corpus_auditor_detects_sprawl() {
        let dir = tempdir().unwrap();
        let tenancy = dir.path().join("tenancy");
        std::fs::create_dir_all(&tenancy).unwrap();
        std::fs::write(
            tenancy.join("rule.md"),
            "---\ncanonical_authority: true\n---\n# Rule",
        )
        .unwrap();

        let report = CorpusAuditor::audit_repository(dir.path(), 180).unwrap();
        assert_eq!(report.unauthorized_ssot_claims.len(), 1);
        assert!(report.unauthorized_ssot_claims[0].contains("tenancy/rule.md"));
    }
}
