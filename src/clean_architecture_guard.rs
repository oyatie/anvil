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

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::git_manager::PrDiffContext;

/// Cross-unit facade bypasses present in Anvil's own tree.
///
/// Exact, not a ceiling. A `<=` bound is slack, and slack is what lets a newly
/// introduced defect land under cover of an existing one; the count that fell
/// silently is the count nobody notices. Lowering this is the work -- each one
/// is a unit reaching into another's interior, and each is an edge that has to
/// go before these units could ever be separated.
pub const FACADE_BYPASSES_IN_ANVIL: usize = 18;

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
/// Whether a source line is an import statement (Rust `use`/`extern crate`,
/// TS/JS `import`, Python `from ... import`). Comments and strings are not
/// dependency edges, however many layer names they mention.
fn is_import_line(trimmed: &str) -> bool {
    let t = trimmed.trim_start();
    if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') || t.starts_with('#') {
        return false;
    }
    let t = t
        .strip_prefix("pub(crate) ")
        .or_else(|| t.strip_prefix("pub(super) "))
        .or_else(|| t.strip_prefix("pub "))
        .unwrap_or(t);
    t.starts_with("use ")
        || t.starts_with("extern crate ")
        || t.starts_with("import ")
        || t.starts_with("from ")
}

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

/// The unit a file belongs to: its directory directly under `src/`.
///
/// A unit is the thing that owns a set of faces. `src/shape/core/glob.rs` and
/// `src/shape/facade/measure.rs` are both `shape`; whether one may import the
/// other is an internal question. Whether `git_manager` may import either is
/// not.
fn unit_of(file_path: &str) -> Option<String> {
    let norm = file_path.replace('\\', "/");
    let mut parts = norm.split('/').filter(|p| !p.is_empty() && *p != ".");
    // Repo-relative (`src/shape/...`) and tree-relative (`shape/...`) paths
    // both occur: PR diffs carry the former, `evaluate_source_tree` the latter.
    let first = parts.next()?;
    let unit = if first == "src" { parts.next()? } else { first };
    if unit.ends_with(".rs") {
        return None; // a loose file directly under src/ owns no faces
    }
    Some(unit.to_string())
}

/// The unit and face an import reaches into, when it names another unit's
/// inner face.
///
/// This is the rule the four faces exist to create, and the one the layer
/// checks above cannot express: reaching *inward* is legitimate within a
/// unit and forbidden across units. `core`, `ports` and `adapters` are a
/// unit's private interior; `facade` is the only importable face. Without
/// this, faces are directory names that constrain nothing -- which is what
/// let `git_manager` bind to `change_delivery::adapters::git_vcs`.
fn cross_unit_bypass(import_line: &str, importing_file: &str) -> Option<(String, String)> {
    let own = unit_of(importing_file);
    // `crate::<unit>::<face>` is the only spelling that can name another
    // unit's interior; `super::` and `self::` are unit-internal by
    // construction and `::`-rooted external crates are not ours to judge.
    let rest = import_line.split("crate::").nth(1)?;
    let mut seg = rest.split("::");
    let unit = seg.next()?.trim();
    let face = seg
        .next()?
        .trim()
        .trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
    if !matches!(
        face,
        "core" | "ports" | "adapters" | "adapter" | "domain" | "application"
    ) {
        return None;
    }
    if own.as_deref() == Some(unit) {
        return None; // a unit may reach into its own interior
    }
    Some((unit.to_string(), face.to_string()))
}

fn layer_name(layer: Option<ArchLayer>) -> &'static str {
    match layer {
        Some(ArchLayer::Core) => "CORE/DOMAIN",
        Some(ArchLayer::Ports) => "PORTS/APPLICATION",
        Some(ArchLayer::Adapters) => "ADAPTERS",
        Some(ArchLayer::Facade) => "FACADE/REST",
        None => "UNLAYERED",
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
                r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?\b(?:adapters?|adapter[-_]\w+|facade|rest)\b"#,
                "ADAPTERS/FACADE",
                "Core/Domain layer must never import from external Adapters or Facade layers",
            ),
            (
                r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?\b(?:ports?|application)\b"#,
                "PORTS/APPLICATION",
                "Core/Domain layer must never import from Ports/Application layers",
            ),
        ];

        let ports_forbidden_imports = [(
            r#"(?i)(?:use\s+|import\s+.*?from\s+['"]).*?\b(?:adapters?|adapter[-_]\w+|facade|rest)\b"#,
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
                // Only an import statement can create a dependency edge. A
                // comment containing "because ports -> core" matched the
                // unanchored `use\s+` through "beca|use" on Anvil's own tree.
                // A cross-unit binding is an edge however it is spelled. The
                // one in `git_manager` is an expression, not a `use`, so the
                // import-line filter below would hide it. Checked first, and
                // deliberately outside that filter.
                // `code_only` on the line, not just the tree: `evaluate_source_tree`
                // strips whole files, but a PR diff arrives as raw text and this
                // guard's own comment naming `crate::x::adapters::Y` was reported
                // as a violation of a unit called `x`. A line whose construct does
                // not terminate blanks to its end, which suppresses rather than
                // fabricates.
                let code = crate::source_scan::code_only(trimmed);
                if let Some((unit, face)) = cross_unit_bypass(code.trim(), &current_file) {
                    violations.push(ArchViolation {
                        file_path: current_file.clone(),
                        source_layer: layer_name(current_layer).to_string(),
                        target_layer: format!("{unit}::{face}"),
                        description: format!(
                            "reaches past `{unit}`'s facade into its `{face}`; only a \
                             unit's facade is importable from outside it"
                        ),
                        snippet: trimmed.to_string(),
                    });
                }

                // The layer-direction rules below are regex matches over an
                // unanchored `use\s+`, which matched the "use" inside
                // "because" on this very tree. They stay import-line-only.
                if !is_import_line(trimmed) {
                    continue;
                }

                match current_layer {
                    Some(ArchLayer::Core) => {
                        for (pattern, target_layer, desc) in &core_forbidden_imports {
                            if let Ok(re) = Regex::new(pattern)
                                && re.is_match(trimmed)
                            {
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
                    Some(ArchLayer::Ports) => {
                        for (pattern, target_layer, desc) in &ports_forbidden_imports {
                            if let Ok(re) = Regex::new(pattern)
                                && re.is_match(trimmed)
                            {
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
                    // Adapters and Facade sit outermost within their own unit:
                    // they may depend inward, so there is no forbidden
                    // direction to check for them here.
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
                    // Say exactly what was measured. "Verified ... 100% intact"
                    // over 8 layered files of 256 is a claim about the 248 the
                    // classifier never saw (I2).
                    format!(
                        "Clean Architecture: 0 layer-boundary violations across \
                         {files_classified} layered file(s) of {files_inspected} examined in \
                         {scope}; {} file(s) belong to no recognised layer and were not measured.",
                        files_inspected.saturating_sub(*files_classified)
                    )
                } else {
                    // The denominator belongs on this branch too. A findings
                    // list alone says nothing about the files the classifier
                    // never saw, and "18 violations" reads as a complete
                    // account of the tree when it is an account of 55 files.
                    format!(
                        "Clean Architecture layer boundary violations ({} items) across \
                         {files_classified} layered file(s) of {files_inspected} examined in \
                         {scope}; {} file(s) belong to no recognised layer and were not \
                         measured. {}",
                        violations.len(),
                        files_inspected.saturating_sub(*files_classified),
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
    fn layer_tokens_match_whole_words_not_substrings() {
        // Observed on Anvil's own tree: `pub use report::Finding;` in a core
        // module matched `ports?` through "re|port", and `rest` would match
        // "forest". A layer name is a path segment, not a substring.
        let guard = CleanArchitectureGuard::new();
        let clean = PrDiffContext {
            repo: "oyatie/anvil".to_string(),
            pr_number: 204,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/shape/core/mod.rs\n+ pub use report::Finding;\n+ use crate::forest::Tree;\n+ use crate::supporting::Thing;".to_string(),
            changed_files: vec!["src/shape/core/mod.rs".to_string()],
            is_incremental: false,
        };
        let report = guard.evaluate_architecture(&clean).expect("Evaluates");
        assert!(
            report.is_clean,
            "substring hits must not be violations: {:?}",
            report.violations
        );
        assert!(report.measurement.is_measured());

        let dirty = PrDiffContext {
            diff_content: "+++ b/src/shape/core/mod.rs\n+ use crate::ports::TreeSource;"
                .to_string(),
            ..clean
        };
        let report = guard.evaluate_architecture(&dirty).expect("Evaluates");
        assert_eq!(
            report.violations.len(),
            1,
            "a real ports import must still fire"
        );
        assert_eq!(report.violations[0].target_layer, "PORTS/APPLICATION");
    }

    #[test]
    fn comments_and_strings_are_not_dependency_edges() {
        let guard = CleanArchitectureGuard::new();
        let ctx = PrDiffContext {
            repo: "oyatie/anvil".to_string(),
            pr_number: 205,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/shape/core/dependency.rs\n+ // denied target (facade -> ports, because ports -> core)\n+ /// an adapters face is where adapters live\n+ let msg = \"use the ports face\";\n+ use super::tree::TreeSource;".to_string(),
            changed_files: vec!["src/shape/core/dependency.rs".to_string()],
            is_incremental: false,
        };
        let report = guard.evaluate_architecture(&ctx).expect("Evaluates");
        assert!(report.is_clean, "{:?}", report.violations);
        assert!(report.measurement.is_measured());
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
