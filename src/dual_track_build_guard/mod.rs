//! Whether a repository's Cargo and Buck2 build graphs are kept in step.
//!
//! # The defect this module carried
//!
//! `is_synchronized` began as `true` and was only ever lowered by a drift
//! check that ran when a Buck2 track was already present. On a repository with
//! no `BUCK` and no `reindeer.toml` the check was skipped entirely and the
//! guard returned a pass -- printing `PASSED` in the same sentence as
//! `Buck2 hermetic RBE track ready = false`.
//!
//! That is a green for the exact absence the guard exists to detect, and it
//! was green on anvil itself, the repository with no Buck2 track at all. Its
//! only test provisioned a `BUCK` file first, so the vacuous arm was never
//! executed.
//!
//! # The distinction the type now forces
//!
//! A pass and an absence are different answers and this repository already has
//! the vocabulary for it: `Absence::NotProvisioned` names a capability the
//! deployment lacks, and does not withhold a merge. `DualTrackVerdict` makes
//! the three answers unconfusable at the type level, so a caller cannot read
//! "nothing to compare" as "compared and agreed" -- which is what a bare
//! `bool` let it do.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

/// What the guard actually established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DualTrackVerdict {
    /// Both tracks exist and this change moved them together.
    Synchronized,
    /// Both tracks exist and this change moved only one of them.
    Drifted { violations: Vec<String> },
    /// There is no second track to be out of step with. NOT a pass: nothing
    /// was compared. Maps to `Absence::NotProvisioned`, which does not
    /// withhold a merge -- no author can add a Buck2 track in the pull request
    /// that trips over its absence.
    NoBuck2Track,
    /// No `Cargo.toml`. Nothing here applies.
    NoCargoTrack,
}

impl DualTrackVerdict {
    /// Whether this verdict is a measurement that succeeded.
    ///
    /// Only `Synchronized`. The two absence arms are deliberately excluded:
    /// treating them as passes is the defect this type exists to remove.
    pub fn is_pass(&self) -> bool {
        matches!(self, DualTrackVerdict::Synchronized)
    }

    /// Whether the guard measured anything at all.
    pub fn measured(&self) -> bool {
        matches!(
            self,
            DualTrackVerdict::Synchronized | DualTrackVerdict::Drifted { .. }
        )
    }

    /// The capability whose absence stopped the measurement, if any.
    pub fn missing_capability(&self) -> Option<&'static str> {
        match self {
            DualTrackVerdict::NoBuck2Track => Some("buck2 build graph (BUCK / reindeer.toml)"),
            DualTrackVerdict::NoCargoTrack => Some("cargo workspace (Cargo.toml)"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualTrackBuildReport {
    /// The answer. `is_synchronized` below is derived from it and kept only so
    /// existing readers keep compiling; new readers should match on this.
    pub verdict: DualTrackVerdict,
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

        // Absence is settled BEFORE any drift check, and returns. Previously
        // the drift check was simply skipped when a track was missing, leaving
        // an `is_synchronized` that had been initialised to `true` and never
        // touched -- a pass nobody ever decided to give.
        let verdict = if !has_cargo {
            DualTrackVerdict::NoCargoTrack
        } else if !has_buck {
            DualTrackVerdict::NoBuck2Track
        } else {
            let cargo_changed = diff_ctx
                .changed_files
                .iter()
                .any(|f| f.ends_with("Cargo.toml"));
            let buck_changed = diff_ctx.changed_files.iter().any(|f| {
                f.ends_with("BUCK") || f.ends_with("reindeer.toml") || f.ends_with("Cargo.lock")
            });
            if cargo_changed && !buck_changed {
                DualTrackVerdict::Drifted {
                    violations: vec![
                        "Cargo.toml modified without updating BUCK / reindeer.toml \
                         dual-track target definitions."
                            .to_string(),
                    ],
                }
            } else {
                DualTrackVerdict::Synchronized
            }
        };

        let summary = match &verdict {
            DualTrackVerdict::Synchronized => {
                "PASSED (dual-track build graph synchronized: cargo and buck2 moved together)"
                    .to_string()
            }
            DualTrackVerdict::Drifted { violations } => {
                format!(
                    "FAILED (dual-track drift detected: {})",
                    violations.join("; ")
                )
            }
            // Deliberately not the word PASSED. The previous summary carried it
            // alongside `ready = false`, and a reader skimming verdict lines
            // had no way to tell this apart from a real measurement.
            DualTrackVerdict::NoBuck2Track | DualTrackVerdict::NoCargoTrack => format!(
                "NOT MEASURED (no {} in this repository, so there is no second \
                 track to be out of step with)",
                verdict.missing_capability().unwrap_or("build track")
            ),
        };

        Ok(DualTrackBuildReport {
            is_synchronized: verdict.is_pass(),
            cargo_track_ready: has_cargo,
            buck2_track_ready: has_buck,
            reindeer_synced: !matches!(verdict, DualTrackVerdict::Drifted { .. }),
            summary,
            verdict,
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
