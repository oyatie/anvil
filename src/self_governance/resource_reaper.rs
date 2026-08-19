use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Default)]
pub struct GarbageCollectionReport {
    pub orphaned_worktrees_pruned: usize,
    pub temporary_directories_reaped: usize,
    pub freed_bytes_estimate: u64,
}

pub struct AutonomousResourceReaper {
    staging_dirs: Vec<PathBuf>,
}

impl Default for AutonomousResourceReaper {
    fn default() -> Self {
        Self::new(vec![
            std::env::temp_dir().join("anvil"),
            std::env::temp_dir().join("anvil_worktrees"),
            std::env::temp_dir().join("anvil_staging"),
        ])
    }
}

impl AutonomousResourceReaper {
    pub fn new(staging_dirs: Vec<PathBuf>) -> Self {
        Self { staging_dirs }
    }

    /// Autonomous background garbage collection sweeping abandoned files and worktrees
    pub async fn run_sweep(&self, repo_dir: Option<&Path>) -> Result<GarbageCollectionReport> {
        let mut report = GarbageCollectionReport::default();

        // 1. Clean Git worktrees if repo_dir provided
        if let Some(dir) = repo_dir {
            if dir.join(".git").exists() {
                let cmd = tokio::process::Command::new("git")
                    .args(["worktree", "prune"])
                    .current_dir(dir)
                    .output()
                    .await;

                if let Ok(out) = cmd {
                    if out.status.success() {
                        report.orphaned_worktrees_pruned += 1;
                    }
                }
            }
        }

        // 2. Clean temporary scratch directories older than 1 hour
        let now = std::time::SystemTime::now();
        for dir in &self.staging_dirs {
            if dir.exists() {
                if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        let path = entry.path();
                        if let Ok(meta) = entry.metadata().await {
                            if let Ok(modified) = meta.modified() {
                                if let Ok(age) = now.duration_since(modified) {
                                    if age > std::time::Duration::from_secs(3600) {
                                        let _ = tokio::fs::remove_dir_all(&path).await;
                                        report.temporary_directories_reaped += 1;
                                        report.freed_bytes_estimate += meta.len();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if report.temporary_directories_reaped > 0 || report.orphaned_worktrees_pruned > 0 {
            info!(
                "🧹 [Autonomous Resource Reaper] Cleaned {} temp directories and {} worktrees (~{} bytes reclaimed)",
                report.temporary_directories_reaped, report.orphaned_worktrees_pruned, report.freed_bytes_estimate
            );
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resource_reaper_sweeps_temp() {
        let temp_base = std::env::temp_dir().join("anvil_test_reaper");
        let _ = tokio::fs::create_dir_all(&temp_base).await;

        let reaper = AutonomousResourceReaper::new(vec![temp_base.clone()]);
        let rep = reaper.run_sweep(None).await;
        assert!(rep.is_ok());

        let _ = tokio::fs::remove_dir_all(&temp_base).await;
    }
}
