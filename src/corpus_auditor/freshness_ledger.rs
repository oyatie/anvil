use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFreshnessRecord {
    pub file_path: String,
    pub days_since_modification: u64,
    pub is_dormant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreshnessLedgerReport {
    pub total_files: usize,
    pub active_files_count: usize,
    pub dormant_files_count: usize,
    pub freshness_ratio: f64,
    pub stale_threshold_days: u64,
    pub dormant_files: Vec<FileFreshnessRecord>,
}

pub struct FreshnessLedger;

impl FreshnessLedger {
    pub const DEFAULT_STALE_THRESHOLD_DAYS: u64 = 180;

    /// Scans the repository filesystem and computes the Freshness Ledger
    pub fn scan_repository(repo_dir: &Path, stale_threshold_days: u64) -> FreshnessLedgerReport {
        let mut total_files: usize = 0;
        let mut dormant_files = Vec::new();

        let mut stack = vec![repo_dir.to_path_buf()];
        let now = SystemTime::now();

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
                    } else if path.is_file() {
                        total_files += 1;
                        let days = if let Ok(meta) = std::fs::metadata(&path) {
                            if let Ok(modified) = meta.modified() {
                                if let Ok(duration) = now.duration_since(modified) {
                                    duration.as_secs() / 86400
                                } else {
                                    0
                                }
                            } else {
                                0
                            }
                        } else {
                            0
                        };

                        let is_dormant = days > stale_threshold_days;
                        if is_dormant {
                            dormant_files.push(FileFreshnessRecord {
                                file_path: rel,
                                days_since_modification: days,
                                is_dormant,
                            });
                        }
                    }
                }
            }
        }

        let dormant_files_count = dormant_files.len();
        let active_files_count = total_files.saturating_sub(dormant_files_count);
        let freshness_ratio = if total_files > 0 {
            active_files_count as f64 / total_files as f64
        } else {
            1.0
        };

        FreshnessLedgerReport {
            total_files,
            active_files_count,
            dormant_files_count,
            freshness_ratio,
            stale_threshold_days,
            dormant_files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_freshness_ledger_computes_ratio() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("file1.rs"), "fn a() {}").unwrap();
        std::fs::write(dir.path().join("file2.rs"), "fn b() {}").unwrap();

        let report = FreshnessLedger::scan_repository(dir.path(), 180);
        assert_eq!(report.total_files, 2);
        assert_eq!(report.dormant_files_count, 0);
        assert_eq!(report.freshness_ratio, 1.0);
    }
}
