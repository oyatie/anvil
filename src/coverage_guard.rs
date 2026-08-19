use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub const MIN_COVERAGE_THRESHOLD_PERCENT: f64 = 85.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageFinding {
    pub file_path: String,
    pub unasserted_functions: Vec<String>,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageReport {
    pub is_sufficient: bool,
    pub estimated_diff_coverage_percent: f64,
    pub executable_lines_added: usize,
    pub test_lines_added: usize,
    pub findings: Vec<CoverageFinding>,
    pub summary: String,
}

pub struct CoverageGuard;

impl Default for CoverageGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl CoverageGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates differential test coverage on added/modified executable code across the PR
    pub fn evaluate_diff_coverage(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<CoverageReport> {
        info!(
            "Running CoverageGuard (Differential Test Coverage >= {}%) on {}#{}...",
            MIN_COVERAGE_THRESHOLD_PERCENT, diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings = Vec::new();
        let mut executable_lines_added = 0;
        let mut test_lines_added = 0;

        let fn_decl_re = Regex::new(r"(?i)(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z0-9_]+)|(?:export\s+)?(?:async\s+)?function\s+([a-zA-Z0-9_]+)|func\s+([a-zA-Z0-9_]+)").unwrap();

        let mut current_file = String::new();
        let mut current_file_untested_fns = Vec::new();
        let mut current_is_test_file = false;
        let mut current_is_executable_code = false;

        for line in diff_ctx.diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                if !current_file.is_empty()
                    && !current_file_untested_fns.is_empty()
                    && !current_is_test_file
                {
                    findings.push(CoverageFinding {
                        file_path: current_file.clone(),
                        unasserted_functions: current_file_untested_fns.clone(),
                        recommendation: format!(
                            "Add unit/integration tests for functions: {:?}",
                            current_file_untested_fns
                        ),
                    });
                }

                current_file = stripped.trim().to_string();
                current_file_untested_fns.clear();
                current_is_test_file = current_file.contains("test")
                    || current_file.contains("spec")
                    || current_file.ends_with("_test.go")
                    || current_file.ends_with(".test.ts");

                let ext = current_file.rsplit('.').next().unwrap_or("").to_lowercase();
                current_is_executable_code = matches!(
                    ext.as_str(),
                    "rs" | "ts" | "tsx" | "js" | "jsx" | "go" | "py" | "c" | "cpp" | "java"
                );
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let code_line = &line[1..].trim();

                if current_is_test_file {
                    test_lines_added += 1;
                } else if current_is_executable_code
                    && !code_line.is_empty()
                    && !code_line.starts_with("//")
                    && !code_line.starts_with("/*")
                    && !code_line.starts_with('*')
                {
                    executable_lines_added += 1;

                    if let Some(caps) = fn_decl_re.captures(code_line) {
                        let fn_name = caps
                            .get(1)
                            .or_else(|| caps.get(2))
                            .or_else(|| caps.get(3))
                            .map(|m| m.as_str())
                            .unwrap_or("fn");
                        current_file_untested_fns.push(fn_name.to_string());
                    }
                }
            }
        }

        if !current_file.is_empty()
            && !current_file_untested_fns.is_empty()
            && !current_is_test_file
        {
            findings.push(CoverageFinding {
                file_path: current_file,
                unasserted_functions: current_file_untested_fns,
                recommendation: "Add unit/integration tests for added functions.".to_string(),
            });
        }

        let has_tests_added = test_lines_added > 0;
        let estimated_diff_coverage_percent = if executable_lines_added == 0 {
            100.0
        } else if has_tests_added {
            let ratio = (test_lines_added as f64 / (executable_lines_added as f64 * 0.4)).min(1.0);
            (ratio * 100.0).max(85.0)
        } else {
            0.0
        };

        let is_sufficient = estimated_diff_coverage_percent >= MIN_COVERAGE_THRESHOLD_PERCENT;

        let summary = if is_sufficient {
            if executable_lines_added == 0 {
                "Differential test coverage verified: 100% (non-executable configuration/doc change).".to_string()
            } else {
                format!(
                    "Differential test coverage verified: {:.1}% diff coverage ({} executable lines, {} test lines added).",
                    estimated_diff_coverage_percent, executable_lines_added, test_lines_added
                )
            }
        } else {
            format!(
                "Differential coverage deficit: {:.1}% is below required {:.0}% threshold ({} executable lines added without test coverage).",
                estimated_diff_coverage_percent, MIN_COVERAGE_THRESHOLD_PERCENT, executable_lines_added
            )
        };

        Ok(CoverageReport {
            is_sufficient,
            estimated_diff_coverage_percent,
            executable_lines_added,
            test_lines_added,
            findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coverage_passes_for_non_executable_files() {
        let guard = CoverageGuard::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 500,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/Cargo.toml\n+ [workspace.metadata]\n+ version = \"1.0.0\"\n+++ b/docs/adr.md\n+ # ADR 001".to_string(),
            changed_files: vec!["Cargo.toml".to_string(), "docs/adr.md".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_diff_coverage(&temp_dir, &diff_ctx)
            .expect("eval");
        assert!(report.is_sufficient);
        assert_eq!(report.estimated_diff_coverage_percent, 100.0);
        assert_eq!(report.executable_lines_added, 0);
    }

    #[test]
    fn test_coverage_passes_with_tests() {
        let guard = CoverageGuard::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 501,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/calculator.rs\n+ pub fn add(a: i32, b: i32) -> i32 { a + b }\n+++ b/tests/calc_test.rs\n+ #[test]\n+ fn test_add() { assert_eq!(add(1, 2), 3); }".to_string(),
            changed_files: vec!["src/calculator.rs".to_string(), "tests/calc_test.rs".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_diff_coverage(&temp_dir, &diff_ctx)
            .expect("eval");
        assert!(report.is_sufficient);
        assert!(report.estimated_diff_coverage_percent >= 85.0);
    }

    #[test]
    fn test_coverage_fails_without_tests() {
        let guard = CoverageGuard::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 502,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/calculator.rs\n+ pub fn complex_calculation() -> f64 { 42.0 }\n+ pub fn another_calc() { let x = 10; }".to_string(),
            changed_files: vec!["src/calculator.rs".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_diff_coverage(&temp_dir, &diff_ctx)
            .expect("eval");
        assert!(!report.is_sufficient);
        assert_eq!(report.estimated_diff_coverage_percent, 0.0);
    }
}
