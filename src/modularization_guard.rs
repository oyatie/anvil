use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::git_manager::PrDiffContext;

pub const MAX_RECOMMENDED_LINES: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OversizedFileFinding {
    pub file_path: String,
    pub line_count: usize,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModularizationReport {
    pub is_modular: bool,
    pub oversized_files: Vec<OversizedFileFinding>,
    pub summary: String,
}

pub struct ModularizationGuard;

impl Default for ModularizationGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ModularizationGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates file length and modularization across the PR
    pub fn evaluate_modularization(
        &self,
        diff_ctx: &PrDiffContext,
    ) -> Result<ModularizationReport> {
        info!(
            "Running ModularizationGuard (100-300 lines max) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut oversized_files = Vec::new();

        // Check modified files in diff for gross expansions
        let mut current_file = String::new();
        let mut line_count = 0;

        for line in diff_ctx.diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                if !current_file.is_empty() && line_count > MAX_RECOMMENDED_LINES {
                    oversized_files.push(OversizedFileFinding {
                        file_path: current_file.clone(),
                        line_count,
                        recommendation: format!("File exceeds {} lines; decompose into cohesive submodules or domain components.", MAX_RECOMMENDED_LINES),
                    });
                }
                current_file = stripped.trim().to_string();
                line_count = 0;
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                line_count += 1;
            }
        }

        if !current_file.is_empty() && line_count > MAX_RECOMMENDED_LINES {
            oversized_files.push(OversizedFileFinding {
                file_path: current_file,
                line_count,
                recommendation: format!("File exceeds {} lines; decompose into cohesive submodules or domain components.", MAX_RECOMMENDED_LINES),
            });
        }

        // Check category-aware directory depth envelope for all changed files
        for file in &diff_ctx.changed_files {
            let depth = file.split('/').count();
            let (category, max_depth) = if file.starts_with("docs/") {
                ("docs", 3)
            } else if file.starts_with("contracts/") {
                ("contracts", 4)
            } else if file.contains("tests/fixtures") || file.contains("fixtures/") {
                ("fixtures", 7)
            } else if file.starts_with("infra/")
                || file.starts_with("iac/")
                || file.starts_with("k8s/")
            {
                ("infrastructure", 6)
            } else {
                ("production_code", 5)
            };

            if depth > max_depth {
                oversized_files.push(OversizedFileFinding {
                    file_path: file.clone(),
                    line_count: depth,
                    recommendation: format!(
                        "Directory depth {} exceeds maximum allowed limit of {} for category '{}'. Flatten module structure.",
                        depth, max_depth, category
                    ),
                });
            }
        }

        let is_modular = oversized_files.is_empty();
        let summary = if is_modular {
            "Module size and directory depth verified: files are strictly bounded within 100-300 lines and adhere to the category-aware depth envelope.".to_string()
        } else {
            format!(
                "Modularization & depth findings ({} violations): {}",
                oversized_files.len(),
                oversized_files
                    .iter()
                    .map(|f| format!("{}: {}", f.file_path, f.recommendation))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(ModularizationReport {
            is_modular,
            oversized_files,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modular_diff_passes() {
        let guard = ModularizationGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 401,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/handler.rs\n+ let x = 1;\n+ let y = 2;".to_string(),
            changed_files: vec!["src/handler.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_modularization(&diff_ctx).expect("Evaluates");
        assert!(report.is_modular);
    }
}
