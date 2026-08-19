use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod service_graph;
pub use service_graph::{CrossServiceFinding, ServiceGraphValidator};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceImpactReport {
    pub is_compatible: bool,
    pub breaking_findings: Vec<CrossServiceFinding>,
    pub summary: String,
}

pub struct CrossServiceImpactEngine {
    validator: ServiceGraphValidator,
}

impl Default for CrossServiceImpactEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CrossServiceImpactEngine {
    pub fn new() -> Self {
        let validator = ServiceGraphValidator::new();
        Self { validator }
    }

    /// 100% Deterministic evaluation of monorepo cross-service blast radius
    pub fn evaluate_cross_service_impact(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<ServiceImpactReport> {
        info!(
            "Running CrossServiceImpactEngine (Deterministic Monorepo Blast-Radius Engine) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut breaking_findings = Vec::new();

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            let lines: Vec<&str> = file_diff.lines().collect();
            let mut current_file = "api.yaml".to_string();
            if let Some(first_line) = lines.first() {
                if let Some(path) = first_line.split_whitespace().last() {
                    current_file = path.trim_start_matches("b/").to_string();
                }
            }

            let file_findings = self
                .validator
                .evaluate_service_contracts(&current_file, file_diff);
            breaking_findings.extend(file_findings);
        }

        let is_compatible = breaking_findings.is_empty();
        let summary = if is_compatible {
            "✅ PASSED (Cross-service wire contract compatibility verified across all monorepo microservices)".to_string()
        } else {
            format!(
                "❌ FAILED ({} cross-service breaking wire contract change(s) detected)",
                breaking_findings.len()
            )
        };

        Ok(ServiceImpactReport {
            is_compatible,
            breaking_findings,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_service_nominal() {
        let engine = CrossServiceImpactEngine::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn endpoint() {}".to_string(),
            changed_files: vec!["src/api.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = engine
            .evaluate_cross_service_impact(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_compatible);
    }
}
