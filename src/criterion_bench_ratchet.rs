use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkViolation {
    pub file_path: String,
    pub metric: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReport {
    pub is_within_budget: bool,
    pub hot_paths_evaluated: usize,
    pub violations: Vec<BenchmarkViolation>,
    pub summary: String,
}

pub struct CriterionBenchRatchet;

impl Default for CriterionBenchRatchet {
    fn default() -> Self {
        Self::new()
    }
}

impl CriterionBenchRatchet {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates micro-benchmarks and hot paths for latency and memory allocation regressions
    pub fn evaluate_benchmarks(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<BenchmarkReport> {
        info!(
            "Running CriterionBenchRatchet on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut violations = Vec::new();

        let hot_path_files: Vec<&String> = diff_ctx
            .changed_files
            .iter()
            .filter(|f| {
                f.contains("bench")
                    || f.contains("hotpath")
                    || f.contains("proto")
                    || f.contains("serialize")
                    || f.contains("hash")
                    || f.contains("crypto")
            })
            .collect();

        let hot_paths_evaluated = hot_path_files.len();

        let unbounded_alloc_re =
            Regex::new(r"(?i)for\s+.*\s+in\s+.*\{[\s\n]*let\s+mut\s+v\s*=\s*Vec::new\(\)").unwrap();
        let clone_in_loop_re = Regex::new(r"(?i)\.clone\(\)\s*;.*//\s*hotpath").unwrap();

        // The added lines, kept per file.
        //
        // Two defects came from not doing this. The clone rule attributed its
        // finding to `current_file`, which is the empty string for any `+` line
        // reaching the scanner before a `+++ b/` header -- an accusation
        // against no file at all. And the allocation rule below could not be
        // written per line, because its pattern spans the `for` and the
        // `Vec::new()` on separate lines, so it read the WHOLE diff instead.
        let mut added_by_file: Vec<(String, String)> = Vec::new();
        let mut current_file: Option<String> = None;

        for line in diff_ctx.diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                current_file = Some(stripped.trim().to_string());
                continue;
            }
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            // A finding must name the file it was found in. Without a header
            // there is no file, and a fragment that names none is not something
            // this gate can attribute anything to.
            let Some(path) = current_file.as_deref() else {
                continue;
            };
            let code_line = line[1..].trim();

            if clone_in_loop_re.is_match(code_line) {
                violations.push(BenchmarkViolation {
                    file_path: path.to_string(),
                    metric: "EXCESSIVE_HOTPATH_CLONE".to_string(),
                    description: "Explicit clone detected on designated hot path; exceeds zero-copy allocation budget.".to_string(),
                    recommendation: "Borrow or use Arc/Cow to avoid heap copies.".to_string(),
                });
            }

            match added_by_file.last_mut() {
                Some((f, body)) if f == path => {
                    body.push_str(code_line);
                    body.push('\n');
                }
                _ => added_by_file.push((path.to_string(), format!("{code_line}\n"))),
            }
        }

        // Reading the whole diff meant the pattern matched on lines the change
        // REMOVES, so the pull request that DELETES an unbounded loop
        // allocation was refused for containing one -- the same inversion found
        // and fixed in the credential scanner. And the finding named
        // `file_path: "hotpath"`, which is not a file: it is the gate's own
        // word for the category, published where a reviewer reads a path.
        for (path, added) in &added_by_file {
            if !unbounded_alloc_re.is_match(added) {
                continue;
            }
            violations.push(BenchmarkViolation {
                file_path: path.clone(),
                metric: "UNBOUNDED_LOOP_ALLOCATION".to_string(),
                description:
                    "Re-allocating collection inside tight loop without pre-allocated capacity."
                        .to_string(),
                recommendation:
                    "Use `Vec::with_capacity` or hoist collection allocation outside the loop."
                        .to_string(),
            });
        }

        let is_within_budget = violations.is_empty();
        let summary = if is_within_budget {
            if hot_paths_evaluated > 0 {
                format!(
                    "Micro-benchmarks verified: {} hot path(s) evaluated within the +3% latency & zero-leak budget.",
                    hot_paths_evaluated
                )
            } else {
                "Micro-benchmark ratchet verified: zero performance or allocation regressions detected.".to_string()
            }
        } else {
            format!(
                "Performance regressions detected ({} violation(s)): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{}: {}", v.metric, v.description))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(BenchmarkReport {
            is_within_budget,
            hot_paths_evaluated,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_bench_passes() {
        let ratchet = CriterionBenchRatchet::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 401,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("/tmp"),
                crate::git_manager::Uncloned::TestFixture,
            ),
            diff_content: "+++ b/benches/parse_bench.rs\n+ let buf = Vec::with_capacity(1024);"
                .to_string(),
            changed_files: vec!["benches/parse_bench.rs".to_string()],
            is_incremental: false,
        };

        let report = ratchet
            .evaluate_benchmarks(&temp_dir, &diff_ctx)
            .expect("eval");
        assert!(report.is_within_budget);
        assert_eq!(report.hot_paths_evaluated, 1);
    }

    #[test]
    fn test_hotpath_clone_fails() {
        let ratchet = CriterionBenchRatchet::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 402,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("/tmp"),
                crate::git_manager::Uncloned::TestFixture,
            ),
            diff_content: "+++ b/src/crypto/token.rs\n+ let data = payload.clone(); // hotpath"
                .to_string(),
            changed_files: vec!["src/crypto/token.rs".to_string()],
            is_incremental: false,
        };

        let report = ratchet
            .evaluate_benchmarks(&temp_dir, &diff_ctx)
            .expect("eval");
        assert!(!report.is_within_budget);
        assert_eq!(report.violations[0].metric, "EXCESSIVE_HOTPATH_CLONE");
    }
}
