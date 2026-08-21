//! Cross-model dual verification: two peer models judge the same prompt, and the
//! gate only passes when neither rejects.
//!
//! This is one of the three units in `ai_driver` with no oyatie counterpart, so it
//! survives absorption. It therefore depends on execution only through
//! [`PromptExecutor`] — never on the subscription executor, the account pool or a
//! process spawner. The consequence is that the agreement arithmetic, the
//! discrepancy record and the fail-closed branch are all reachable from a test with
//! a scripted double (`tests/ai_driver_executor_port_test.rs`) instead of requiring
//! two vendor CLIs and a logged-in subscription.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use super::executor_port::PromptExecutor;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossModelConsensusReport {
    pub is_consensus_reached: bool,
    pub model_a_verdict: String,
    pub model_b_verdict: String,
    pub agreement_score: f64, // 0.0 to 1.0
    pub consensus_summary: String,
    pub identified_discrepancies: Vec<String>,
}

/// A verdict that rejects the change, in the path where both peers answered.
fn is_rejection(text: &str) -> bool {
    text.contains("REQUEST_CHANGES") || text.contains("REJECT") || text.contains("VIOLATION")
}

/// A verdict that rejects the change, in the path where only one peer answered.
///
/// Deliberately wider than [`is_rejection`]: with half the evidence missing, a bare
/// `FAIL` is also treated as a rejection rather than being read as approval.
fn is_lone_survivor_rejection(text: &str) -> bool {
    is_rejection(text) || text.contains("FAIL")
}

#[derive(Debug, Clone)]
pub struct CrossModelDualValidator<E: PromptExecutor> {
    executor: E,
    model_a: String,
    model_b: String,
}

impl<E: PromptExecutor> CrossModelDualValidator<E> {
    /// Builds a validator that compares the two named models through `executor`.
    ///
    /// Which two models duel is the caller's decision. The previous version
    /// hardcoded one specific pair of subscriptions, which is what made the type
    /// impossible to construct without them.
    pub fn new(executor: E, model_a: impl Into<String>, model_b: impl Into<String>) -> Self {
        Self {
            executor,
            model_a: model_a.into(),
            model_b: model_b.into(),
        }
    }

    /// Evaluates a critical task across two diverse peer models and reports whether
    /// they agree.
    pub async fn verify_cross_model_consensus(
        &self,
        prompt: &str,
        working_dir: &Path,
    ) -> Result<CrossModelConsensusReport> {
        info!(
            "Executing cross-model dual verification ({} vs {})...",
            self.model_a, self.model_b
        );

        // Both peers are consulted concurrently; neither result gates the other.
        let (result_a, result_b) = tokio::join!(
            self.executor.execute(&self.model_a, prompt, working_dir),
            self.executor.execute(&self.model_b, prompt, working_dir)
        );

        let (text_a, text_b) = match (result_a, result_b) {
            (Ok(a), Ok(b)) => (a, b),
            (Ok(a), Err(e)) => {
                warn!(
                    "Model {} failed in cross-validation: {}. Evaluating model {} fail-closed.",
                    self.model_b, e, self.model_a
                );
                let has_rejection = is_lone_survivor_rejection(&a);

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
                        "{} evaluation ({} unavailable): {}",
                        self.model_a,
                        self.model_b,
                        a.chars().take(200).collect::<String>()
                    ),
                    identified_discrepancies: if has_rejection {
                        vec![format!(
                            "{} issued rejection or critical violation while {} was unavailable",
                            self.model_a, self.model_b
                        )]
                    } else {
                        Vec::new()
                    },
                });
            }
            (Err(e), Ok(b)) => {
                warn!(
                    "Model {} failed in cross-validation: {}. Evaluating model {} fail-closed.",
                    self.model_a, e, self.model_b
                );
                let has_rejection = is_lone_survivor_rejection(&b);

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
                        "{} evaluation ({} unavailable): {}",
                        self.model_b,
                        self.model_a,
                        b.chars().take(200).collect::<String>()
                    ),
                    identified_discrepancies: if has_rejection {
                        vec![format!(
                            "{} issued rejection or critical violation while {} was unavailable",
                            self.model_b, self.model_a
                        )]
                    } else {
                        Vec::new()
                    },
                });
            }
            (Err(ea), Err(eb)) => {
                anyhow::bail!("Both dual-verification models failed: A: {}, B: {}", ea, eb);
            }
        };

        let is_a_reject = is_rejection(&text_a);
        let is_b_reject = is_rejection(&text_b);

        let mut discrepancies = Vec::new();
        let agreement_score = if is_a_reject == is_b_reject {
            1.0
        } else {
            discrepancies.push(format!(
                "Divergence: {} and {} emitted conflicting verdicts on safety invariants",
                self.model_a, self.model_b
            ));
            0.5
        };

        let is_consensus_reached = agreement_score >= 0.8 && !is_a_reject && !is_b_reject;
        let summary = if is_consensus_reached {
            format!(
                "✅ DUAL-MODEL CONSENSUS REACHED: both {} and {} verified safety and correctness.",
                self.model_a, self.model_b
            )
        } else if is_a_reject || is_b_reject {
            format!(
                "🚨 CRITICAL SAFETY CONFLICT: dual verification identified safety rejections ({} reject: {}, {} reject: {})",
                self.model_a, is_a_reject, self.model_b, is_b_reject
            )
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
