use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use super::provider::{ModelExecutionConfig, ModelProvider};
use super::router::SubscriptionExecutor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgenticStage {
    Recon,
    Planning,
    PlanReview,
    ArchitectSpec,
    SpecReview,
    Implementation,
    CodeReviewAudit,
    GitOps,
}

impl AgenticStage {
    pub fn display_name(&self) -> &'static str {
        match self {
            AgenticStage::Recon => "1. Scout / Recon / Research",
            AgenticStage::Planning => "2. High-Level Architectural Planning",
            AgenticStage::PlanReview => "3. Adversarial Plan Review & Critic",
            AgenticStage::ArchitectSpec => "4. Architecture & Typed Contract Specification",
            AgenticStage::SpecReview => "5. Specification & Threat Model Audit",
            AgenticStage::Implementation => "6. Pure Rust Code Synthesis & Implementation",
            AgenticStage::CodeReviewAudit => "7. 16-Lens Code Review & Hyperscaler Consensus",
            AgenticStage::GitOps => "8. GitOps & Speculative Merge Queue Enlistment",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageModelPair {
    pub primary: ModelExecutionConfig,
    pub fallback: ModelExecutionConfig,
}

#[derive(Debug, Clone)]
pub struct EnterpriseAgenticPipelineRouter {
    executor: SubscriptionExecutor,
}

impl Default for EnterpriseAgenticPipelineRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl EnterpriseAgenticPipelineRouter {
    pub fn new() -> Self {
        Self {
            executor: SubscriptionExecutor::new(),
        }
    }

    /// Returns the deterministic model dispatch pair (Primary + Fallback) for a specific agentic pipeline stage
    pub fn get_stage_config(stage: AgenticStage) -> StageModelPair {
        match stage {
            // Stage 0: Scout/Recon -> Primary: GPT-5.3-Codex-Spark, Fallback: Gemini 3.7 Flash Medium
            AgenticStage::Recon => StageModelPair {
                primary: ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.3-codex-spark".to_string()),
                    reasoning_effort: "medium".to_string(),
                    print_timeout_secs: 180,
                },
                fallback: ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.7-flash".to_string()),
                    reasoning_effort: "medium".to_string(),
                    print_timeout_secs: 180,
                },
            },

            // Stage 1: Planning -> Primary: Claude -p at Fable 5 xhigh, Fallback: Codex exec at GPT-5.6-Sol xhigh
            AgenticStage::Planning => StageModelPair {
                primary: ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("fable-5".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                fallback: ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
            },

            // Stage 2: Plan Review / Critic -> Primary: Opus 5 High, Fallback: GPT-5.6-Sol High
            AgenticStage::PlanReview => StageModelPair {
                primary: ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("opus-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                fallback: ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
            },

            // Stage 3: Architect & Spec -> Primary: Claude -p at Fable 5 xhigh, Fallback: Codex exec at GPT-5.6-Sol xhigh
            AgenticStage::ArchitectSpec => StageModelPair {
                primary: ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("fable-5".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                fallback: ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
            },

            // Stage 4: Spec Review / Threat Model Audit -> Primary: Opus 5 High, Fallback: GPT-5.6-Sol High
            AgenticStage::SpecReview => StageModelPair {
                primary: ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("opus-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                fallback: ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
            },

            // Stage 5: Implementation / Code Synthesis -> Primary: Grok 4.6 xhigh, Fallback: Gemini 3.6 Flash High
            AgenticStage::Implementation => StageModelPair {
                primary: ModelExecutionConfig {
                    provider: ModelProvider::XAiGrok,
                    specific_model: Some("grok-4.6".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                fallback: ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.6-flash".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 300,
                },
            },

            // Stage 6: Code Review, Audit & Consensus -> Primary: Opus 5 High, Fallback: GPT-5.6-Sol High
            AgenticStage::CodeReviewAudit => StageModelPair {
                primary: ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("opus-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                fallback: ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
            },

            // Stage 7: GitOps -> Deterministic engine
            AgenticStage::GitOps => StageModelPair {
                primary: ModelExecutionConfig::default(),
                fallback: ModelExecutionConfig::default(),
            },
        }
    }

    /// Executes prompt for a specific pipeline stage with automatic graceful fallover
    pub async fn dispatch_stage(
        &self,
        stage: AgenticStage,
        prompt: &str,
        working_dir: &Path,
    ) -> Result<String> {
        let pair = Self::get_stage_config(stage);
        info!(
            "🚀 [Enterprise Pipeline] Stage: {} | Primary Dispatch: {} ({:?})",
            stage.display_name(),
            pair.primary.resolved_model(),
            pair.primary.provider
        );

        match self
            .executor
            .execute_prompt(prompt, working_dir, &pair.primary)
            .await
        {
            Ok(result) if !result.trim().is_empty() => {
                info!(
                    "✅ [Enterprise Pipeline] Stage: {} completed via Primary Model ({})",
                    stage.display_name(),
                    pair.primary.resolved_model()
                );
                Ok(result)
            }
            Err(e) => {
                warn!(
                    "⚠️ [Enterprise Pipeline] Primary model ({}) failed for stage {}: {}. Triggering Fallover to {} ({:?})...",
                    pair.primary.resolved_model(),
                    stage.display_name(),
                    e,
                    pair.fallback.resolved_model(),
                    pair.fallback.provider
                );

                self.executor
                    .execute_prompt(prompt, working_dir, &pair.fallback)
                    .await
                    .with_context(|| {
                        format!(
                            "Both Primary ({}) and Fallback ({}) models failed for stage {}",
                            pair.primary.resolved_model(),
                            pair.fallback.resolved_model(),
                            stage.display_name()
                        )
                    })
            }
            Ok(_) => {
                warn!(
                    "⚠️ [Enterprise Pipeline] Primary model returned empty output. Triggering Fallback to {}...",
                    pair.fallback.resolved_model()
                );
                self.executor
                    .execute_prompt(prompt, working_dir, &pair.fallback)
                    .await
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_model_pairs_match_hyperscaler_topology() {
        // Recon
        let recon = EnterpriseAgenticPipelineRouter::get_stage_config(AgenticStage::Recon);
        assert_eq!(recon.primary.resolved_model(), "gpt-5.3-codex-spark");
        assert_eq!(recon.fallback.resolved_model(), "gemini-3.7-flash");

        // Planning
        let plan = EnterpriseAgenticPipelineRouter::get_stage_config(AgenticStage::Planning);
        assert_eq!(plan.primary.resolved_model(), "fable-5");
        assert_eq!(plan.primary.reasoning_effort, "xhigh");
        assert_eq!(plan.fallback.resolved_model(), "gpt-5.6-sol");
        assert_eq!(plan.fallback.reasoning_effort, "xhigh");

        // Plan Review
        let plan_rev = EnterpriseAgenticPipelineRouter::get_stage_config(AgenticStage::PlanReview);
        assert_eq!(plan_rev.primary.resolved_model(), "opus-5");
        assert_eq!(plan_rev.primary.reasoning_effort, "high");
        assert_eq!(plan_rev.fallback.resolved_model(), "gpt-5.6-sol");

        // Implementation
        let impl_stage =
            EnterpriseAgenticPipelineRouter::get_stage_config(AgenticStage::Implementation);
        assert_eq!(impl_stage.primary.resolved_model(), "grok-4.6");
        assert_eq!(impl_stage.primary.reasoning_effort, "xhigh");
        assert_eq!(impl_stage.fallback.resolved_model(), "gemini-3.6-flash");

        // Code Review & Audit
        let code_rev =
            EnterpriseAgenticPipelineRouter::get_stage_config(AgenticStage::CodeReviewAudit);
        assert_eq!(code_rev.primary.resolved_model(), "opus-5");
        assert_eq!(code_rev.fallback.resolved_model(), "gpt-5.6-sol");
    }
}
