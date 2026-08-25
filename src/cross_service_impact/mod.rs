use crate::git_manager::diff_context::{BothSides, diffs_by_path};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod contract_scan;
pub use contract_scan::{CrossServiceFinding, NO_CONSUMER_REGISTRY};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceImpactReport {
    pub is_compatible: bool,
    pub breaking_findings: Vec<CrossServiceFinding>,
    pub summary: String,
}

pub struct CrossServiceImpactEngine;

impl Default for CrossServiceImpactEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CrossServiceImpactEngine {
    pub fn new() -> Self {
        Self
    }

    /// Reports required schema fields a changed wire contract lost.
    ///
    /// The impacted consumer set is not part of the answer: see
    /// [`NO_CONSUMER_REGISTRY`].
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

        for file in diffs_by_path(&diff_ctx.diff_content) {
            // The path is the one the diff states. It used to default to the
            // literal "api.yaml", a plausible path this gate published
            // as the location of a finding that was not found there.
            //
            // `raw` -- this rule compares the two sides of the diff on purpose: a
            // required field disappearing IS the finding, so it needs the markers.

            breaking_findings.extend(contract_scan::removed_required_fields(
                &file.path,
                // The only rule in the corpus whose SUBJECT is the removal: a
                // `required` field that disappears is the breaking change, so
                // it cannot work from additions. Naming the reason is what
                // keeps that deliberate rather than accidental.
                file.both_sides(BothSides::ContractComparesRemovedFields),
            ));
        }

        let is_compatible = breaking_findings.is_empty();
        let contracts_read: Vec<&String> = diff_ctx
            .changed_files
            .iter()
            .filter(|f| contract_scan::is_wire_contract(f))
            .collect();

        let summary = if is_compatible {
            format!(
                "No required schema field was removed from the {} changed wire contract(s) read ({}). Nothing else about compatibility is measured, and {}.",
                contracts_read.len(),
                if contracts_read.is_empty() {
                    "none in this diff".to_string()
                } else {
                    contracts_read
                        .iter()
                        .map(|f| f.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                NO_CONSUMER_REGISTRY
            )
        } else {
            format!(
                "{} required schema field(s) removed from a wire contract: {}. {}.",
                breaking_findings.len(),
                breaking_findings
                    .iter()
                    .map(|f| format!("{} lost `{}`", f.contract_file, f.removed_required_field))
                    .collect::<Vec<_>>()
                    .join("; "),
                NO_CONSUMER_REGISTRY
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
    fn a_diff_touching_no_wire_contract_is_compatible() {
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
        assert!(rep.summary.contains("none in this diff"));
    }
}
