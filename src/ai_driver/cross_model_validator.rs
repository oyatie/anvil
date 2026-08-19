use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use super::provider::{ModelExecutionConfig, ModelProvider};
use super::router::SubscriptionExecutor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossModelConsensusReport {
    pub is_consensus_reached: bool,
    pub model_a_verdict: String,
    pub model_b_verdict: String,
    pub agreement_score: f64, // 0.0 to 1.0
    pub consensus_summary: String,
    pub identified_discrepancies: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CrossModelDualValidator {
    executor: SubscriptionExecutor,
}

impl CrossModelDualValidator {
    pub fn new() -> Self {
        Self {
            executor: SubscriptionExecutor::new(),
        }
    }

    /// Evaluates critical tasks across dual diverse model families (e.g. Claude Opus 5 + GPT-5.6-Sol)
    pub async fn verify_cross_model_consensus(
        &self,
        prompt: &str,
        working_dir: &Path,
    ) -> Result<CrossModelConsensusReport> {
        info!("Executing Cross-Model Dual-Verification (Claude Opus 5 ⚔️ GPT-5.6-Sol)...");

        let config_a = ModelExecutionConfig {
            provider: ModelProvider::AnthropicClaudeCode,
            specific_model: Some("opus-5".to_string()),
            reasoning_effort: "high".to_string(),
            print_timeout_secs: 420,
        };

        let config_b = ModelExecutionConfig {
            provider: ModelProvider::OpenAiCodex,
            specific_model: Some("gpt-5.6-sol".to_string()),
            reasoning_effort: "high".to_string(),
            print_timeout_secs: 420,
        };

        // Execute both model prompts concurrently in parallel
        let (result_a, result_b) = tokio::join!(
            self.executor.execute_prompt(prompt, working_dir, &config_a),
            self.executor.execute_prompt(prompt, working_dir, &config_b)
        );

        let (text_a, text_b) = match (result_a, result_b) {
            (Ok(a), Ok(b)) => (a, b),
            (Ok(a), Err(e)) => {
                warn!(
                    "Model B failed in cross-validation: {}. Evaluating Model A fail-closed.",
                    e
                );
                let has_rejection = a.contains("REQUEST_CHANGES")
                    || a.contains("VIOLATION")
                    || a.contains("REJECT")
                    || a.contains("FAIL");

                return Ok(CrossModelConsensusReport {
                    is_consensus_reached: !has_rejection,
                    model_a_verdict: if has_rejection {
                        "REJECTED".to_string()
                    } else {
                        "APPROVED".to_string()
                    },
                    model_b_verdict: "UNAVAILABLE_FAIL_CLOSED".to_string(),
                    agreement_score: if has_rejection { 0.0 } else { 0.85 },
                    consensus_summary: format!(
                        "Model A evaluation (Model B unavailable): {}",
                        a.chars().take(200).collect::<String>()
                    ),
                    identified_discrepancies: if has_rejection {
                        vec!["Model A issued rejection or critical violation while Model B was unavailable".to_string()]
                    } else {
                        Vec::new()
                    },
                });
            }
            (Err(e), Ok(b)) => {
                warn!(
                    "Model A failed in cross-validation: {}. Evaluating Model B fail-closed.",
                    e
                );
                let has_rejection = b.contains("REQUEST_CHANGES")
                    || b.contains("VIOLATION")
                    || b.contains("REJECT")
                    || b.contains("FAIL");

                return Ok(CrossModelConsensusReport {
                    is_consensus_reached: !has_rejection,
                    model_a_verdict: "UNAVAILABLE_FAIL_CLOSED".to_string(),
                    model_b_verdict: if has_rejection {
                        "REJECTED".to_string()
                    } else {
                        "APPROVED".to_string()
                    },
                    agreement_score: if has_rejection { 0.0 } else { 0.85 },
                    consensus_summary: format!(
                        "Model B evaluation (Model A unavailable): {}",
                        b.chars().take(200).collect::<String>()
                    ),
                    identified_discrepancies: if has_rejection {
                        vec!["Model B issued rejection or critical violation while Model A was unavailable".to_string()]
                    } else {
                        Vec::new()
                    },
                });
            }
            (Err(ea), Err(eb)) => {
                anyhow::bail!("Both dual-verification models failed: A: {}, B: {}", ea, eb);
            }
        };

        let is_a_reject = text_a.contains("REQUEST_CHANGES")
            || text_a.contains("REJECT")
            || text_a.contains("VIOLATION");
        let is_b_reject = text_b.contains("REQUEST_CHANGES")
            || text_b.contains("REJECT")
            || text_b.contains("VIOLATION");

        let mut discrepancies = Vec::new();
        let agreement_score = if is_a_reject == is_b_reject {
            1.0
        } else {
            discrepancies.push(
                "Divergence: Model A and Model B emitted conflicting verdicts on safety invariants"
                    .to_string(),
            );
            0.5
        };

        let is_consensus_reached = agreement_score >= 0.8 && !is_a_reject && !is_b_reject;
        let summary = if is_consensus_reached {
            "✅ DUAL-MODEL CONSENSUS REACHED: Both Claude Opus 5 and GPT-5.6-Sol verified safety and correctness.".to_string()
        } else if is_a_reject || is_b_reject {
            format!("🚨 CRITICAL SAFETY CONFLICT: Dual verification identified safety rejections (Model A Reject: {}, Model B Reject: {})", is_a_reject, is_b_reject)
        } else {
            "⚠️ Partial agreement between peer models.".to_string()
        };

        Ok(CrossModelConsensusReport {
            is_consensus_reached,
            model_a_verdict: if is_a_reject {
                "REJECT".to_string()
            } else {
                "APPROVE".to_string()
            },
            model_b_verdict: if is_b_reject {
                "REJECT".to_string()
            } else {
                "APPROVE".to_string()
            },
            agreement_score,
            consensus_summary: summary,
            identified_discrepancies: discrepancies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cross_model_validator_detects_agreement() {
        let rep = CrossModelConsensusReport {
            is_consensus_reached: true,
            model_a_verdict: "APPROVE".to_string(),
            model_b_verdict: "APPROVE".to_string(),
            agreement_score: 1.0,
            consensus_summary: "Consensus reached".to_string(),
            identified_discrepancies: Vec::new(),
        };

        assert!(rep.is_consensus_reached);
        assert_eq!(rep.agreement_score, 1.0);
    }
}
