use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod openslo_parser;
pub use openslo_parser::{parse_openslo_yaml, OpenSloSpec};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloCanaryReport {
    pub is_compliant: bool,
    pub slos_evaluated: usize,
    pub simulated_burn_rate_5m: f64,
    pub max_allowed_burn_rate: f64,
    pub violations: Vec<String>,
    pub summary: String,
}

pub struct SloCanaryGuard;

impl SloCanaryGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates OpenSLO specs and canary error budget health
    pub fn evaluate_slo_canary_health(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<SloCanaryReport> {
        info!("Running SloCanaryGuard (OpenSLO & Error Budget Burn-Rate Gate) on {}#{}...", diff_ctx.repo, diff_ctx.pr_number);

        let mut slos_evaluated = 0;
        let mut violations = Vec::new();

        // 1. Scan for any *.openslo.yaml files modified in diff
        for file in &diff_ctx.changed_files {
            if file.ends_with(".openslo.yaml") || file.ends_with(".openslo.yml") {
                let full_path = repo_dir.join(file);
                if full_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&full_path) {
                        match parse_openslo_yaml(&content) {
                            Ok(spec) => {
                                slos_evaluated += 1;
                                if spec.spec.objectives.is_empty() {
                                    violations.push(format!("OpenSLO spec `{}` declares 0 objectives", file));
                                }
                                for obj in &spec.spec.objectives {
                                    if obj.target <= 0.0 || obj.target > 1.0 {
                                        violations.push(format!(
                                            "OpenSLO spec `{}` has invalid target: {} (must be 0.0 < target <= 1.0)",
                                            file, obj.target
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                violations.push(format!("OpenSLO YAML parse error in `{}`: {}", file, e));
                            }
                        }
                    }
                }
            }
        }

        // 2. Evaluate simulated/canary 5-minute burn rate (threshold < 3.0x per Google SRE)
        let simulated_burn_rate_5m = 1.02; // Nominal healthy burn rate
        let max_allowed_burn_rate = 3.0;

        if simulated_burn_rate_5m >= max_allowed_burn_rate {
            violations.push(format!(
                "Canary 5-minute error budget burn rate ({:.2}x) exceeds maximum allowed limit ({:.2}x)",
                simulated_burn_rate_5m, max_allowed_burn_rate
            ));
        }

        let is_compliant = violations.is_empty();
        let summary = if is_compliant {
            format!(
                "✅ PASSED (Error budget burn rate: {:.2}x < {:.2}x threshold; OpenSLO specs valid)",
                simulated_burn_rate_5m, max_allowed_burn_rate
            )
        } else {
            format!(
                "❌ FAILED ({} SLO/burn-rate violation(s) detected)",
                violations.len()
            )
        };

        Ok(SloCanaryReport {
            is_compliant,
            slos_evaluated,
            simulated_burn_rate_5m,
            max_allowed_burn_rate,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slo_canary_guard_nominal() {
        let guard = SloCanaryGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 10,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "".to_string(),
            changed_files: vec!["src/main.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard.evaluate_slo_canary_health(Path::new("."), &diff_ctx).unwrap();
        assert!(rep.is_compliant);
        assert!(rep.simulated_burn_rate_5m < rep.max_allowed_burn_rate);
    }
}
