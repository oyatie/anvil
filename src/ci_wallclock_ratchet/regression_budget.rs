use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimizationSuggestion {
    pub category: String,
    pub diagnosis: String,
    pub remedy: String,
    pub estimated_wallclock_savings_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiDurationSnapshot {
    pub pr_wallclock_seconds: u64,
    pub trunk_baseline_seconds: u64,
    pub billable_compute_cost_usd: f64,
    pub trunk_baseline_cost_usd: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressionDecision {
    pub is_acceptable: bool,
    pub is_genuinely_justified_with_adr: bool,
    pub wallclock_delta_pct: f64,
    pub cost_delta_pct: f64,
    pub suggestions: Vec<OptimizationSuggestion>,
    pub explanation: String,
}

pub struct RegressionBudgetEvaluator;

impl RegressionBudgetEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates CI wallclock and compute economics with nuanced diagnostic decomposition and actionable optimization guidance
    pub fn evaluate_regression(
        &self,
        snapshot: &CiDurationSnapshot,
        has_adr_justification: bool,
        diff_content: &str,
    ) -> RegressionDecision {
        let wallclock_delta_pct = if snapshot.trunk_baseline_seconds > 0 {
            ((snapshot.pr_wallclock_seconds as f64 - snapshot.trunk_baseline_seconds as f64)
                / snapshot.trunk_baseline_seconds as f64)
                * 100.0
        } else {
            0.0
        };

        let cost_delta_pct = if snapshot.trunk_baseline_cost_usd > 0.0 {
            ((snapshot.billable_compute_cost_usd - snapshot.trunk_baseline_cost_usd)
                / snapshot.trunk_baseline_cost_usd)
                * 100.0
        } else {
            0.0
        };

        let mut suggestions = Vec::new();

        // 1. Diagnose compile-time macro bloat
        if diff_content.contains("features = [\"full\"]") {
            suggestions.push(OptimizationSuggestion {
                category: "Compile-Time / Macro Trimming".to_string(),
                diagnosis: "Heavy dependency feature `[\"full\"]` added, inducing macro expansion overhead across compiler stages.".to_string(),
                remedy: "Trim feature flags to only referenced sub-modules (e.g. `features = [\"parsing\", \"printing\"]`).".to_string(),
                estimated_wallclock_savings_seconds: 22,
            });
        }

        // 2. Diagnose sequential test execution
        if diff_content.contains("std::thread::sleep") || diff_content.contains("tokio::time::sleep") {
            suggestions.push(OptimizationSuggestion {
                category: "Test Harness / Concurrency".to_string(),
                diagnosis: "Hardcoded sleep timers detected in test suites, extending worker wallclock sequentially.".to_string(),
                remedy: "Replace `sleep()` with event-driven `tokio::sync::Notify` or channel readiness polling.".to_string(),
                estimated_wallclock_savings_seconds: 15,
            });
        }

        // 3. Diagnose un-cached build.rs
        if diff_content.contains("fn main()") && diff_content.contains("build.rs") && !diff_content.contains("cargo:rerun-if-changed") {
            suggestions.push(OptimizationSuggestion {
                category: "Build Script Caching".to_string(),
                diagnosis: "`build.rs` runs unconditionally on every compile due to missing rerun triggers.".to_string(),
                remedy: "Emit `println!(\"cargo:rerun-if-changed=src/\")` to enable incremental compiler cache hits.".to_string(),
                estimated_wallclock_savings_seconds: 18,
            });
        }

        let is_over_budget = wallclock_delta_pct > 15.0 || cost_delta_pct > 20.0;

        if is_over_budget && !has_adr_justification {
            let explanation = if !suggestions.is_empty() {
                format!(
                    "⚠️ CI Wallclock/Cost increased by +{:.1}% (+{}s). Feature may be justified, but {} efficiency optimization(s) identified to recover wallclock.",
                    wallclock_delta_pct,
                    snapshot.pr_wallclock_seconds.saturating_sub(snapshot.trunk_baseline_seconds),
                    suggestions.len()
                )
            } else {
                format!(
                    "⚠️ CI Wallclock increased by +{:.1}% (+{}s). If functionally necessary, attach an approved Living ADR justification.",
                    wallclock_delta_pct,
                    snapshot.pr_wallclock_seconds.saturating_sub(snapshot.trunk_baseline_seconds)
                )
            };

            return RegressionDecision {
                is_acceptable: false,
                is_genuinely_justified_with_adr: false,
                wallclock_delta_pct,
                cost_delta_pct,
                suggestions,
                explanation,
            };
        }

        let explanation = if has_adr_justification && is_over_budget {
            format!(
                "✨ PASSED (Wallclock expansion of +{:.1}% explicitly justified by Living Architecture Decision Record)",
                wallclock_delta_pct
            )
        } else {
            format!(
                "✅ PASSED (CI wallclock {}s and compute cost ${:.3} within trunk efficiency envelope)",
                snapshot.pr_wallclock_seconds, snapshot.billable_compute_cost_usd
            )
        };

        RegressionDecision {
            is_acceptable: true,
            is_genuinely_justified_with_adr: has_adr_justification,
            wallclock_delta_pct,
            cost_delta_pct,
            suggestions,
            explanation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provides_actionable_optimization_suggestions_on_regression() {
        let eval = RegressionBudgetEvaluator::new();
        let snapshot = CiDurationSnapshot {
            pr_wallclock_seconds: 220, // +22%
            trunk_baseline_seconds: 180,
            billable_compute_cost_usd: 0.12,
            trunk_baseline_cost_usd: 0.10,
        };
        let diff = "+ syn = { version = \"2.0\", features = [\"full\"] }\n+ tokio::time::sleep(Duration::from_secs(5)).await;";
        let decision = eval.evaluate_regression(&snapshot, false, diff);

        assert!(!decision.is_acceptable);
        assert_eq!(decision.suggestions.len(), 2);
        assert_eq!(decision.suggestions[0].category, "Compile-Time / Macro Trimming");
    }

    #[test]
    fn test_accepts_justified_regression_with_adr() {
        let eval = RegressionBudgetEvaluator::new();
        let snapshot = CiDurationSnapshot {
            pr_wallclock_seconds: 230,
            trunk_baseline_seconds: 180,
            billable_compute_cost_usd: 0.12,
            trunk_baseline_cost_usd: 0.10,
        };
        let decision = eval.evaluate_regression(&snapshot, true, "");
        assert!(decision.is_acceptable);
        assert!(decision.is_genuinely_justified_with_adr);
    }
}
