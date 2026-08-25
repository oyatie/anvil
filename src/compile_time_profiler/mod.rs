use crate::git_manager::diff_context::diffs_by_path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod heavy_deps;
pub use heavy_deps::{HeavyDependencyFinding, HeavyDependencyScanner};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileProfileReport {
    pub is_lean: bool,
    pub findings: Vec<HeavyDependencyFinding>,
    pub summary: String,
}

pub struct CompileTimeProfiler {
    scanner: HeavyDependencyScanner,
}

impl Default for CompileTimeProfiler {
    fn default() -> Self {
        Self::new()
    }
}

impl CompileTimeProfiler {
    pub fn new() -> Self {
        let scanner = HeavyDependencyScanner::new();
        Self { scanner }
    }

    /// 100% Deterministic evaluation of compile-heavy dependencies and build.rs scripts
    pub fn evaluate_compile_profile(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<CompileProfileReport> {
        info!(
            "Running CompileTimeProfiler (Deterministic Heavy-Compile & Macro Profiler) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings = Vec::new();

        for file in diffs_by_path(&diff_ctx.diff_content) {
            // The path is the one the diff states. It used to default to the
            // literal "Cargo.toml", a plausible path this gate published
            // as the location of a finding that was not found there.
            //
            // `all` -- additions plus the context they sit in, removals excluded. The
            // rule asks what the file says after this change, and a line the
            // change DELETES is not part of that.

            let file_findings = self.scanner.scan_heavy_dependencies(&file.path, &file.all);
            findings.extend(file_findings);
        }

        let is_lean = findings.is_empty();
        let summary = if is_lean {
            "✅ PASSED (Zero un-budgeted compile-time macro dependencies or un-cached build.rs scripts)".to_string()
        } else {
            format!(
                "⚠️ CAUTION ({} heavy compile-time dependency/script addition(s) detected)",
                findings.len()
            )
        };

        Ok(CompileProfileReport {
            is_lean,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_profiler_nominal() {
        let profiler = CompileTimeProfiler::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ serde = { version = \"1.0\" }".to_string(),
            changed_files: vec!["Cargo.toml".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = profiler
            .evaluate_compile_profile(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_lean);
    }
}
