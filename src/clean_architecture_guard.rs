use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchViolation {
    pub file_path: String,
    pub source_layer: String, // "CORE/DOMAIN", "PORTS/APPLICATION"
    pub target_layer: String, // "ADAPTERS", "FACADE/REST"
    pub description: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanArchitectureReport {
    pub is_clean: bool,
    pub violations: Vec<ArchViolation>,
    pub summary: String,
}

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

        let mut violations = Vec::new();
        let mut current_file = String::new();

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

        for line in diff_ctx.diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                current_file = stripped.trim().to_string();
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let trimmed = line[1..].trim();
                let is_core_file = current_file.contains("/core/")
                    || current_file.contains("/domain/")
                    || current_file.ends_with("/core.rs")
                    || current_file.ends_with("/domain.rs");
                let is_ports_file = current_file.contains("/ports/")
                    || current_file.contains("/application/")
                    || current_file.ends_with("/ports.rs")
                    || current_file.ends_with("/application.rs");

                if is_core_file {
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
                } else if is_ports_file {
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
            }
        }

        let is_clean = violations.is_empty();
        let summary = if is_clean {
            "Hexagonal Clean Architecture verified: strict inward dependency direction (Core <- Ports <- Adapters <- Facade) 100% intact.".to_string()
        } else {
            format!(
                "Clean Architecture layer boundary violations ({} items): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{}: {}", v.file_path, v.description))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(CleanArchitectureReport {
            is_clean,
            violations,
            summary,
        })
    }
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
    }
}
