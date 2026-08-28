//! Core -> Ports -> Adapters -> Facade boundary enforcement.
//!
//! This guard is deliberately runnable against *two* kinds of input:
//!
//!   * a pull request diff from some other repository
//!     ([`CleanArchitectureGuard::evaluate_architecture`]), and
//!   * a source tree on disk, including Anvil's own
//!     ([`CleanArchitectureGuard::evaluate_source_tree`] /
//!     [`CleanArchitectureGuard::self_conformance`]).
//!
//! Both funnel through the same analysis, so the standard Anvil applies to other
//! people is the standard it reports on itself.
//!
//! The report carries an explicit [`ArchMeasurement`] third state. A tree in
//! which no file belongs to any recognised layer has not been shown to be clean —
//! it has not been measured at all. Reporting that as `is_clean = true` would be
//! absent evidence dressed as a pass, the same failure the repo's
//! `GateStatus::NotMeasured` vocabulary exists to prevent (see
//! `src/pre_merge_guard/report.rs`, invariant I1).
//!
//! Three of Anvil's units carry faces (`change_delivery`, `ratchet`, `shape`),
//! so `self_conformance()` now measures rather than declining to.
//!
//! # Faces are sealed, not just named
//!
//! Layer ordering within a unit is only half the rule, and it was the half
//! this guard enforced. It classified a file by ITS OWN path, so a file
//! belonging to no layer -- which is most of this tree -- could bind to any
//! other unit's interior and be reported as clean. It was, 0 violations
//! across 55 classified files, while `git_manager` held a direct reference to
//! `change_delivery::adapters::git_vcs`.
//!
//! The missing rule is that a unit's `core`, `ports` and `adapters` are its
//! interior and only its `facade` is importable from outside it. Without it,
//! faces are directory names that constrain nothing. With it, the four faces
//! do the one job they exist for: a unit's dependencies become a property of
//! its facade alone, which is what makes a dependency graph acyclic and a
//! unit separable.
//!
//! An edge counts however it is spelled. The `git_manager` binding is an
//! expression, not a `use`, so an import-line filter hid it.

mod analyze;
mod paths;
mod report;
mod scan;
mod source_tree;

use std::fs;
use std::path::Path;

use anyhow::Result;
use tracing::{info, warn};

use crate::git_manager::PrDiffContext;

use source_tree::collect_rust_files;

pub use report::{ArchLayer, ArchMeasurement, ArchViolation, CleanArchitectureReport};

/// Cross-unit facade bypasses present in Anvil's own tree.
///
/// Exact, not a ceiling. A `<=` bound is slack, and slack is what lets a newly
/// introduced defect land under cover of an existing one; the count that fell
/// silently is the count nobody notices. Lowering this is the work -- each one
/// is a unit reaching into another's interior, and each is an edge that has to
/// go before these units could ever be separated.
///
/// It rose from 18 to 19 when the parser stopped reading one path per line:
/// `src/bin/occupancy.rs` binds to `change_delivery::core`, and a binary
/// reaching into the library's interior is the same bypass as any other. A
/// ratchet that climbs because the check got sharper is not a regression, and
/// the distinction is only legible if it is written down.
pub const FACADE_BYPASSES_IN_ANVIL: usize = 8;

/// Anvil's own source tree, as it stood at build time.
pub const ANVIL_SOURCE_TREE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");

pub struct CleanArchitectureGuard;

impl Default for CleanArchitectureGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl CleanArchitectureGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates Clean Architecture layer boundaries (Core, Ports, Adapters, Facade) across PR diffs
    pub fn evaluate_architecture(
        &self,
        diff_ctx: &PrDiffContext,
    ) -> Result<CleanArchitectureReport> {
        info!(
            "Running CleanArchitectureGuard (Core -> Ports -> Adapters -> Facade) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        Ok(analyze::analyze_unified_diff(
            &diff_ctx.diff_content,
            format!("{}#{}", diff_ctx.repo, diff_ctx.pr_number),
            &source_tree::workspace_members(&diff_ctx.repo_working_dir),
        ))
    }

    /// Evaluates the same layer boundaries against a source tree on disk.
    ///
    /// This is the entrypoint that makes the guard applicable to Anvil itself:
    /// it reads the tree, renders it in the unified-diff shape the analysis
    /// already understands, and runs the identical rules.
    pub fn evaluate_source_tree(&self, root: &Path) -> Result<CleanArchitectureReport> {
        let scope = format!("source tree {}", root.display());

        if !root.is_dir() {
            let reason = format!(
                "source tree {} is not readable on this host",
                root.display()
            );
            return Ok(CleanArchitectureReport {
                is_clean: false,
                violations: Vec::new(),
                summary: format!("Clean Architecture NOT MEASURED for {scope}: {reason}."),
                measurement: ArchMeasurement::NotMeasured {
                    reason,
                    files_inspected: 0,
                },
                scope,
            });
        }

        let files = collect_rust_files(root)?;
        let base = root.parent().unwrap_or(root);
        let mut diff = String::new();
        for file in &files {
            let rel = file.strip_prefix(base).unwrap_or(file).to_string_lossy();
            diff.push_str("+++ b/");
            diff.push_str(&rel);
            diff.push('\n');
            // Comments and string literals stripped, byte offsets preserved.
            // A dependency edge is created by code; a sentence about one is not.
            // This is also what makes an inline `crate::x::adapters::Y`
            // reference visible without re-admitting the `beca|use` false
            // positive that `is_import_line` was introduced to stop.
            let body = crate::source_scan::code_only(&fs::read_to_string(file).unwrap_or_default());
            for line in body.lines() {
                diff.push('+');
                diff.push_str(line);
                diff.push('\n');
            }
        }

        Ok(analyze::analyze_unified_diff(
            &diff,
            scope,
            &source_tree::workspace_members(root),
        ))
    }

    /// Runs the guard against Anvil's own source tree and records the finding.
    ///
    /// Anvil holds other repositories to this standard; this is the same check
    /// turned inward. The result today is `NotMeasured` — Anvil has no
    /// core/ports/adapters/facade layering — and that is reported as-is.
    pub fn self_conformance(&self) -> Result<CleanArchitectureReport> {
        let report = self.evaluate_source_tree(Path::new(ANVIL_SOURCE_TREE))?;
        match &report.measurement {
            ArchMeasurement::NotMeasured {
                reason,
                files_inspected,
            } => warn!(
                "CleanArchitectureGuard self-conformance: NOT MEASURED on Anvil's own tree \
                 ({files_inspected} file(s) read): {reason}"
            ),
            ArchMeasurement::Measured {
                files_inspected,
                files_classified,
            } => {
                if report.is_clean {
                    info!(
                        "CleanArchitectureGuard self-conformance: clean across \
                         {files_classified} layered file(s) of {files_inspected} read."
                    );
                } else {
                    warn!(
                        "CleanArchitectureGuard self-conformance: {} violation(s) in Anvil's own \
                         tree ({files_classified} layered file(s) of {files_inspected} read): {}",
                        report.violations.len(),
                        report.summary
                    );
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests;
