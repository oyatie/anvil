use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualTrackBuildReport {
    pub is_synchronized: bool,
    pub cargo_track_ready: bool,
    pub buck2_track_ready: bool,
    pub reindeer_synced: bool,
    pub summary: String,
}

pub struct DualTrackBuildGuard;

impl Default for DualTrackBuildGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl DualTrackBuildGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates Cargo + Buck2 dual-track build graph synchronization and hermetic readiness
    pub fn evaluate_dual_track_build(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<DualTrackBuildReport> {
        info!(
            "Running DualTrackBuildGuard on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let has_cargo = repo_dir.join("Cargo.toml").exists();
        let has_buck = repo_dir.join("BUCK").exists()
            || repo_dir.join("BUCK2.meta").exists()
            || repo_dir.join("reindeer.toml").exists();

        let touches_build_graph = diff_ctx.changed_files.iter().any(|f| {
            f.ends_with("Cargo.toml")
                || f.ends_with("Cargo.lock")
                || f.ends_with("BUCK")
                || f.ends_with("reindeer.toml")
        });

        let mut reindeer_synced = true;
        let mut violations = Vec::new();

        if touches_build_graph && has_cargo && has_buck {
            // Check if Cargo.toml was changed without reindeer / BUCK update
            let cargo_changed = diff_ctx
                .changed_files
                .iter()
                .any(|f| f.ends_with("Cargo.toml"));
            let buck_changed = diff_ctx.changed_files.iter().any(|f| {
                f.ends_with("BUCK") || f.ends_with("reindeer.toml") || f.ends_with("Cargo.lock")
            });

            if cargo_changed && !buck_changed {
                reindeer_synced = false;
                violations.push("Cargo.toml modified without updating BUCK / reindeer.toml dual-track target definitions.".to_string());
            }
        }

        let is_synchronized = violations.is_empty();
        let summary = if is_synchronized {
            format!(
                "✅ PASSED (Dual-track build graph synchronized: Cargo fast path ready, Buck2 hermetic RBE track ready = {})",
                has_buck
            )
        } else {
            format!(
                "❌ FAILED (Dual-track drift detected: {})",
                violations.join("; ")
            )
        };

        Ok(DualTrackBuildReport {
            is_synchronized,
            cargo_track_ready: has_cargo,
            buck2_track_ready: has_buck,
            reindeer_synced,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_dual_track_guard_passes_when_clean() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        std::fs::write(dir.path().join("BUCK"), "# Buck2 root\n").unwrap();

        let guard = DualTrackBuildGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 200,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ let x = 1;".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: dir.path().to_path_buf(),
            is_incremental: false,
            previous_head_sha: None,
        };

        let report = guard
            .evaluate_dual_track_build(dir.path(), &diff_ctx)
            .unwrap();
        assert!(report.is_synchronized);
        assert!(report.cargo_track_ready);
        assert!(report.buck2_track_ready);
    }
}
