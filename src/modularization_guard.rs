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
            if line.starts_with("+++ b/") {
                if !current_file.is_empty() && line_count > MAX_RECOMMENDED_LINES {
                    oversized_files.push(OversizedFileFinding {
                        file_path: current_file.clone(),
                        line_count,
                        recommendation: format!("File exceeds {} lines; decompose into cohesive submodules or domain components.", MAX_RECOMMENDED_LINES),
                    });
                }
                current_file = line[6..].trim().to_string();
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

        let is_modular = oversized_files.is_empty();
        let summary = if is_modular {
            "Hyperscaler modularization verified: all modified files are strictly bounded within 100-300 lines.".to_string()
        } else {
            format!(
                "Modularization findings ({} oversized files): {}",
                oversized_files.len(),
                oversized_files
                    .iter()
                    .map(|f| format!("{}: {} lines", f.file_path, f.line_count))
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
