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
//! Anvil's own `src/` currently contains no core/ports/adapters/facade
//! structure, so `self_conformance()` returns `NotMeasured`. That is the honest
//! result and it is recorded, not suppressed.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::git_manager::PrDiffContext;

/// Anvil's own source tree, as it stood at build time.
pub const ANVIL_SOURCE_TREE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchViolation {
    pub file_path: String,
    pub source_layer: String, // "CORE/DOMAIN", "PORTS/APPLICATION"
    pub target_layer: String, // "ADAPTERS", "FACADE/REST"
    pub description: String,
    pub snippet: String,
}

/// Which architectural layer a file sits in, derived from its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchLayer {
    Core,
    Ports,
    Adapters,
    Facade,
}

/// Whether the guard was actually able to make an architectural claim.
///
/// The third state matters: a scan that classified zero files did not verify
/// anything, and must not collapse into "clean".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArchMeasurement {
    /// At least one file belonged to a recognised layer and was checked.
    Measured {
        files_inspected: usize,
        files_classified: usize,
    },
    /// Nothing to measure: no file in the input belonged to core/ports/
    /// adapters/facade, or the tree could not be read. No claim is made in
    /// either direction.
    NotMeasured {
        reason: String,
        files_inspected: usize,
    },
}

impl Default for ArchMeasurement {
    /// Absent evidence, never a pass.
    fn default() -> Self {
        ArchMeasurement::NotMeasured {
            reason: "no measurement recorded".to_string(),
            files_inspected: 0,
        }
    }
}

impl ArchMeasurement {
    pub fn is_measured(&self) -> bool {
        matches!(self, ArchMeasurement::Measured { .. })
    }

    /// `Some(reason)` when nothing could be measured.
    pub fn not_measured_reason(&self) -> Option<&str> {
        match self {
            ArchMeasurement::NotMeasured { reason, .. } => Some(reason),
            ArchMeasurement::Measured { .. } => None,
        }
    }

    pub fn files_inspected(&self) -> usize {
        match self {
            ArchMeasurement::Measured {
                files_inspected, ..
            } => *files_inspected,
            ArchMeasurement::NotMeasured {
                files_inspected, ..
            } => *files_inspected,
        }
    }

    pub fn files_classified(&self) -> usize {
        match self {
            ArchMeasurement::Measured {
                files_classified, ..
            } => *files_classified,
            ArchMeasurement::NotMeasured { .. } => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanArchitectureReport {
    /// True only when the run measured something *and* found no violations.
    /// An unmeasured run is never clean — check [`Self::measurement`] first.
    pub is_clean: bool,
    pub violations: Vec<ArchViolation>,
    pub summary: String,
    /// What the run was actually able to observe.
    #[serde(default)]
    pub measurement: ArchMeasurement,
    /// What was examined: a PR (`repo#number`) or a source tree path.
    #[serde(default)]
    pub scope: String,
}

pub struct CleanArchitectureGuard;

impl Default for CleanArchitectureGuard {
    fn default() -> Self {
        Self::new()
    }
}

/// Classifies a file path into an architectural layer, or `None` when the path
/// carries no layer information at all.
fn classify_layer(file_path: &str) -> Option<ArchLayer> {
    // Normalise so a tree-relative path such as `core/x.rs` matches the same
    // `/core/` convention a repo-relative path uses.
    let path = format!("/{}", file_path.trim_start_matches('/').to_lowercase());

    if path.contains("/core/")
        || path.contains("/domain/")
        || path.ends_with("/core.rs")
        || path.ends_with("/domain.rs")
    {
        return Some(ArchLayer::Core);
    }
    if path.contains("/ports/")
        || path.contains("/application/")
        || path.ends_with("/ports.rs")
        || path.ends_with("/application.rs")
    {
        return Some(ArchLayer::Ports);
    }
    if path.contains("/adapter") || path.ends_with("/adapters.rs") || path.ends_with("/adapter.rs")
    {
        return Some(ArchLayer::Adapters);
    }
    if path.contains("/facade/")
        || path.contains("/rest/")
        || path.ends_with("/facade.rs")
        || path.ends_with("/rest.rs")
    {
        return Some(ArchLayer::Facade);
    }
    None
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

        Ok(self.analyze_unified_diff(
            &diff_ctx.diff_content,
            format!("{}#{}", diff_ctx.repo, diff_ctx.pr_number),
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
            let body = fs::read_to_string(file).unwrap_or_default();
            for line in body.lines() {
                diff.push('+');
                diff.push_str(line);
                diff.push('\n');
            }
        }

        Ok(self.analyze_unified_diff(&diff, scope))
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

    /// The shared analysis: identical rules whether the input is a foreign PR
    /// or Anvil's own tree.
    fn analyze_unified_diff(&self, diff_content: &str, scope: String) -> CleanArchitectureReport {
        let mut violations = Vec::new();
        let mut current_file = String::new();
        let mut current_layer: Option<ArchLayer> = None;
        let mut files_inspected = 0usize;
        let mut files_classified = 0usize;

        let core_forbidden_imports = [
            (
                r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?(?:adapters?|adapter[-_]\w+|facade|rest)"#,
                "ADAPTERS/FACADE",
                "Core/Domain layer must never import from external Adapters or Facade layers",
            ),
            (
                r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?(?:ports?|application)"#,
                "PORTS/APPLICATION",
                "Core/Domain layer must never import from Ports/Application layers",
            ),
        ];

        let ports_forbidden_imports = [(
            r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?(?:adapters?|adapter[-_]\w+|facade|rest)"#,
            "ADAPTERS/FACADE",
            "Ports/Application layer must never import from concrete Adapters or Facade layers",
        )];

        for line in diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                current_file = stripped.trim().to_string();
                current_layer = classify_layer(&current_file);
                files_inspected += 1;
                if current_layer.is_some() {
                    files_classified += 1;
                }
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let trimmed = line[1..].trim();

                match current_layer {
                    Some(ArchLayer::Core) => {
                        for (pattern, target_layer, desc) in &core_forbidden_imports {
                            if let Ok(re) = Regex::new(pattern) {
                                if re.is_match(trimmed) {
                                    violations.push(ArchViolation {
                                        file_path: current_file.clone(),
                                        source_layer: "CORE/DOMAIN".to_string(),
                                        target_layer: target_layer.to_string(),
                                        description: desc.to_string(),
                                        snippet: trimmed.to_string(),
                                    });
                                }
                            }
                        }
                    }
                    Some(ArchLayer::Ports) => {
                        for (pattern, target_layer, desc) in &ports_forbidden_imports {
                            if let Ok(re) = Regex::new(pattern) {
                                if re.is_match(trimmed) {
                                    violations.push(ArchViolation {
                                        file_path: current_file.clone(),
                                        source_layer: "PORTS/APPLICATION".to_string(),
                                        target_layer: target_layer.to_string(),
                                        description: desc.to_string(),
                                        snippet: trimmed.to_string(),
                                    });
                                }
                            }
                        }
                    }
                    // Adapters and Facade sit outermost: they may depend inward,
                    // so there is no forbidden direction to check for them.
                    Some(ArchLayer::Adapters) | Some(ArchLayer::Facade) | None => {}
                }
            }
        }

        let measurement = if files_classified == 0 {
            ArchMeasurement::NotMeasured {
                reason: format!(
                    "no core/ports/adapters/facade layering found: 0 of {files_inspected} \
                     file(s) examined belong to a recognised layer"
                ),
                files_inspected,
            }
        } else {
            ArchMeasurement::Measured {
                files_inspected,
                files_classified,
            }
        };

        // An unmeasured run is not a clean run.
        let is_clean = violations.is_empty() && measurement.is_measured();

        let summary = match &measurement {
            ArchMeasurement::NotMeasured { reason, .. } => format!(
                "Clean Architecture NOT MEASURED for {scope}: {reason}. No inward-dependency \
                 claim can be made about this tree."
            ),
            ArchMeasurement::Measured {
                files_inspected,
                files_classified,
            } => {
                if violations.is_empty() {
                    format!(
                        "Hexagonal Clean Architecture verified for {scope} across \
                         {files_classified} layered file(s) of {files_inspected} examined: \
                         strict inward dependency direction (Core <- Ports <- Adapters <- Facade) \
                         100% intact."
                    )
                } else {
                    format!(
                        "Clean Architecture layer boundary violations ({} items) in {scope}: {}",
                        violations.len(),
                        violations
                            .iter()
                            .map(|v| format!("{}: {}", v.file_path, v.description))
                            .collect::<Vec<_>>()
                            .join("; ")
                    )
                }
            }
        };

        CleanArchitectureReport {
            is_clean,
            violations,
            summary,
            measurement,
            scope,
        }
    }
}

/// Every `.rs` file under `root`, recursively, sorted for determinism.
/// Build output and hidden directories are skipped.
fn collect_rust_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                if name == "target" || name.starts_with('.') {
                    continue;
                }
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catches_core_importing_adapter() {
        let guard = CleanArchitectureGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 201,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/repos/oyatie/tenancy/core/src/tenant.rs\n+ use crate::adapters::postgres::PgPool;".to_string(),
            changed_files: vec!["repos/oyatie/tenancy/core/src/tenant.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_architecture(&diff_ctx).expect("Evaluates");
        assert!(!report.is_clean);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].source_layer, "CORE/DOMAIN");
    }

    #[test]
    fn test_valid_inward_adapter_import_passes() {
        let guard = CleanArchitectureGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 202,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/repos/console/backend/crates/payroll/adapter-postgres/src/repo.rs\n+ use payroll_domain::PayrollRecord;\n+ use payroll_ports::PayrollStore;".to_string(),
            changed_files: vec!["repos/console/backend/crates/payroll/adapter-postgres/src/repo.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_architecture(&diff_ctx).expect("Evaluates");
        assert!(report.is_clean);
        assert!(report.measurement.is_measured());
    }

    #[test]
    fn test_unlayered_diff_is_not_measured_rather_than_clean() {
        let guard = CleanArchitectureGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 203,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/util.rs\n+ use crate::adapters::pg::Pool;".to_string(),
            changed_files: vec!["src/util.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_architecture(&diff_ctx).expect("Evaluates");
        assert!(!report.is_clean);
        assert!(report.violations.is_empty());
        assert!(report.measurement.not_measured_reason().is_some());
        assert!(report.summary.to_lowercase().contains("not measured"));
    }

    #[test]
    fn test_missing_source_tree_is_not_measured() {
        let guard = CleanArchitectureGuard::new();
        let report = guard
            .evaluate_source_tree(Path::new("/nonexistent/anvil/src"))
            .expect("Evaluates");
        assert!(!report.is_clean);
        assert!(report.measurement.not_measured_reason().is_some());
    }

    #[test]
    fn test_self_conformance_reads_anvils_own_tree() {
        let guard = CleanArchitectureGuard::new();
        let report = guard.self_conformance().expect("Evaluates");
        // Deliberately asserts nothing about Anvil being clean: only that the
        // guard actually read the tree and reported a state it can defend.
        assert!(
            report.measurement.files_inspected() > 0,
            "self-conformance read no files from {ANVIL_SOURCE_TREE}"
        );
        if report.measurement.files_classified() == 0 {
            assert!(!report.is_clean);
            assert!(report.summary.to_lowercase().contains("not measured"));
        }
    }
}
