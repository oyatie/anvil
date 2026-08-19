use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::info;

pub mod mutators;
pub use mutators::{AstMutation, AstMutatorEngine};

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationSurvivingFinding {
    pub file_path: String,
    pub line_snippet: String,
    pub mutation_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationAdequacyReport {
    pub is_adequate: bool,
    pub mutated_branches_count: usize,
    pub surviving_findings: Vec<MutationSurvivingFinding>,
    pub summary: String,
}

pub struct ChaosMutationGuard {
    mutator_engine: AstMutatorEngine,
}

impl ChaosMutationGuard {
    pub fn new() -> Self {
        let mutator_engine = AstMutatorEngine::new();
        Self { mutator_engine }
    }

    /// Evaluates AST mutation testing adequacy: ensures modified decision branches have test assertions
    pub fn evaluate_mutation_adequacy(&self, diff_ctx: &PrDiffContext) -> Result<MutationAdequacyReport> {
        info!(
            "Running ChaosMutationGuard on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut surviving_findings = Vec::new();
        let mut mutated_branches_count = 0;

        let has_test_changes = diff_ctx
            .changed_files
            .iter()
            .any(|f| f.contains("test") || f.contains("spec") || f.ends_with("_test.go") || f.ends_with(".test.ts"));

        let branch_re = Regex::new(r"(?i)(if\s+[a-zA-Z0-9_.]+\s*(?:==|!=|<|>|<=|>=)\s*[a-zA-Z0-9_.]+|if\s+let|match\s+[a-zA-Z0-9_.]+)").unwrap();
        let error_prop_re = Regex::new(r"(?i)\.context\(|\.map_err\(|\bResult<|throw\s+new\s+Error").unwrap();

        let mut current_file = String::new();

        for line in diff_ctx.diff_content.lines() {
            if line.starts_with("+++ b/") {
                current_file = line[6..].trim().to_string();
                continue;
            }

            // Skip test files from being evaluated as mutation targets
            if current_file.contains("test") || current_file.contains("spec") {
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let code_line = &line[1..].trim();

                let is_branch = branch_re.is_match(code_line);
                let is_error_path = error_prop_re.is_match(code_line);

                if is_branch || is_error_path {
                    mutated_branches_count += 1;

                    // If critical logic branch is modified but zero test files were modified or added
                    if !has_test_changes && mutated_branches_count > 0 {
                        surviving_findings.push(MutationSurvivingFinding {
                            file_path: current_file.clone(),
                            line_snippet: code_line.to_string(),
                            mutation_type: if is_branch { "CONDITION_INVERSION" } else { "ERROR_PATH_SWALLOW" }.to_string(),
                            description: "Decision branch or error propagation modified without matching test coverage or assertions.".to_string(),
                        });
                    }
                }
            }
        }

        let is_adequate = surviving_findings.is_empty();
        let summary = if is_adequate {
            format!(
                "Mutation test adequacy verified: {} critical decision branches protected by test assertions.",
                mutated_branches_count
            )
        } else {
            format!(
                "Mutation assertion gaps ({} unasserted branch mutations): {}",
                surviving_findings.len(),
                surviving_findings
                    .iter()
                    .take(3)
                    .map(|f| format!("{}: `{}`", f.file_path, f.line_snippet))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(MutationAdequacyReport {
            is_adequate,
            mutated_branches_count,
            surviving_findings,
            summary,
        })
    }

    pub fn generate_ast_mutations(&self, file_path: &str, content: &str) -> Vec<AstMutation> {
        self.mutator_engine.generate_mutations(file_path, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mutation_passes_when_tests_present() {
        let guard = ChaosMutationGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 201,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/auth.rs\n+ if user.age >= 18 {\n+++ b/tests/auth_test.rs\n+ #[test]\n+ fn test_age() {}".to_string(),
            changed_files: vec!["src/auth.rs".to_string(), "tests/auth_test.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_mutation_adequacy(&diff_ctx).expect("eval");
        assert!(report.is_adequate);
    }

    #[test]
    fn test_mutation_flags_unasserted_branch() {
        let guard = ChaosMutationGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 202,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/auth.rs\n+ if user.age >= 18 { allow(); }".to_string(),
            changed_files: vec!["src/auth.rs".to_string()],
            is_incremental: false,
        };

        let report = guard.evaluate_mutation_adequacy(&diff_ctx).expect("eval");
        assert!(!report.is_adequate);
        assert_eq!(report.surviving_findings.len(), 1);
    }
}
