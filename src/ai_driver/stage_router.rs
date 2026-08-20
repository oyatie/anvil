use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

use super::executor_port::ConfiguredPromptExecutor;
use super::provider::{ModelExecutionConfig, ModelProvider};

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
    IssueTriage,
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
            AgenticStage::CodeReviewAudit => "7. 16-Lens Code Review & Reviewer Consensus",
            AgenticStage::GitOps => "8. GitOps & Speculative Merge Queue Enlistment",
            AgenticStage::IssueTriage => "9. Issue Fate Classification",
        }
    }
}

/// Represents an ordered multi-tier fallback chain for a pipeline stage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageFallbackChain {
    pub stage: AgenticStage,
    pub tiers: Vec<ModelExecutionConfig>,
}

impl StageFallbackChain {
    pub fn primary(&self) -> Option<&ModelExecutionConfig> {
        self.tiers.first()
    }

    pub fn fallbacks(&self) -> &[ModelExecutionConfig] {
        if self.tiers.len() > 1 {
            &self.tiers[1..]
        } else {
            &[]
        }
    }
}

/// Dispatches a prompt down a stage's fallback chain.
///
/// The executor arrives as a port rather than being constructed here. That is the
/// `Rewired` shape the migration ledger records for `ai_driver`: the port survives
/// absorption while today's subscription-CLI adapter is swapped for an
/// oyatie-backed one. It also keeps `stage_router` free of any concrete dependency
/// on the superseded executor, which is what lets `telemetry_ledger` — one of the
/// three novel units — import `AgenticStage` from here without dragging a process
/// spawner along behind it.
#[derive(Clone)]
pub struct StageModelRouter {
    executor: Arc<dyn ConfiguredPromptExecutor>,
}

impl StageModelRouter {
    pub fn new(executor: Arc<dyn ConfiguredPromptExecutor>) -> Self {
        Self { executor }
    }

    /// Returns the multi-tier model fallback chain optimized against DeepSWE benchmarks & cost economics
    pub fn get_stage_fallback_chain(stage: AgenticStage) -> StageFallbackChain {
        let tiers = match stage {
            // Stage 0: Scout/Recon -> Rapid AST discovery, token throughput, low cost
            // Tier 1: GPT-5.3-Codex-Spark (medium)
            // Tier 2: Gemini 3.7 Flash (medium) - 1M+ context window
            // Tier 3: Claude 3.7 Sonnet / Haiku (low)
            // Tier 4: Antigravity default
            AgenticStage::Recon => vec![
                ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.3-codex-spark".to_string()),
                    reasoning_effort: "medium".to_string(),
                    print_timeout_secs: 180,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.7-flash".to_string()),
                    reasoning_effort: "medium".to_string(),
                    print_timeout_secs: 180,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("claude-3-7-sonnet".to_string()),
                    reasoning_effort: "low".to_string(),
                    print_timeout_secs: 180,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.6-flash".to_string()),
                    reasoning_effort: "low".to_string(),
                    print_timeout_secs: 120,
                },
            ],

            // Stage 1: Planning -> Highest DeepSWE Systemic Reasoning & Multi-File Planning
            // Tier 1: Claude Fable 5 (xhigh effort via claude -p) - DeepSWE top tier
            // Tier 2: GPT-5.6-Sol (xhigh effort via codex exec)
            // Tier 3: Claude Opus 5 (high effort)
            // Tier 4: Gemini 3.7 Flash (high effort)
            AgenticStage::Planning => vec![
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("fable-5".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("opus-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.7-flash".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 300,
                },
            ],

            // Stage 2: Plan Review / Adversarial Critic -> DeepSWE Verification, Over-Scoping & Race Detection
            // Tier 1: Claude Opus 5 (high effort) - DeepSWE leader in bug/race detection
            // Tier 2: GPT-5.6-Sol (high effort) - Formal logic & STRIDE verification
            // Tier 3: Claude Fable 5 (high effort)
            // Tier 4: Grok 4.6 (high effort)
            AgenticStage::PlanReview => vec![
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("opus-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("fable-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::XAiGrok,
                    specific_model: Some("grok-4.6".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 300,
                },
            ],

            // Stage 3: Architect & Spec Definition -> Typed Protobuf, Cedar Policies & Rollout Schemas
            // Tier 1: Claude Fable 5 (xhigh effort)
            // Tier 2: GPT-5.6-Sol (xhigh effort)
            // Tier 3: Claude Opus 5 (high effort)
            // Tier 4: Gemini 3.7 Flash (high effort)
            AgenticStage::ArchitectSpec => vec![
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("fable-5".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("opus-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.7-flash".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 300,
                },
            ],

            // Stage 4: Spec Review & Threat Model Audit -> STRIDE Security & Fail-Closed Bounds
            // Tier 1: Claude Opus 5 (high effort)
            // Tier 2: GPT-5.6-Sol (high effort)
            // Tier 3: Grok 4.6 (high effort)
            // Tier 4: Gemini 3.6 Flash (high effort)
            AgenticStage::SpecReview => vec![
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("opus-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::XAiGrok,
                    specific_model: Some("grok-4.6".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 300,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.6-flash".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 300,
                },
            ],

            // Stage 5: Implementation & Code Synthesis -> DeepSWE First-Pass Compilation & Pass@1
            // Tier 1: Grok 4.6 (xhigh effort) - Pure compiled Rust & type safety
            // Tier 2: Gemini 3.6 Flash (high effort) - Rapid cost-effective synthesis
            // Tier 3: GPT-5.6-Sol (xhigh effort)
            // Tier 4: Claude Fable 5 (high effort)
            AgenticStage::Implementation => vec![
                ModelExecutionConfig {
                    provider: ModelProvider::XAiGrok,
                    specific_model: Some("grok-4.6".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.6-flash".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 300,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "xhigh".to_string(),
                    print_timeout_secs: 600,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("fable-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
            ],

            // Stage 6: Code Review, Audit & Consensus -> 16-Lens Review + 5-Cloud Approval
            // Tier 1: Claude Opus 5 (high effort) - DeepSWE top reviewer
            // Tier 2: GPT-5.6-Sol (high effort) - Multi-cloud compliance
            // Tier 3: Claude Fable 5 (high effort)
            // Tier 4: Gemini 3.7 Flash (high effort)
            AgenticStage::CodeReviewAudit => vec![
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("opus-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.6-sol".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("fable-5".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 420,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.7-flash".to_string()),
                    reasoning_effort: "high".to_string(),
                    print_timeout_secs: 300,
                },
            ],

            // Stage 7: GitOps -> Deterministic Engine
            AgenticStage::GitOps => vec![ModelExecutionConfig::default()],

            // Stage 9: Issue fate classification -> high volume, shallow judgement.
            //
            // Deciding an issue's fate (keep / close / dedupe / relabel / escalate) is
            // classification, not repair. The capability that predicts quality here is
            // instruction-following and structured-output reliability, plus long-context
            // recall when the decision is "is this a duplicate of one of the N open
            // issues" -- NOT patch synthesis. SWE-bench-family scores rank patch
            // synthesis, so they are deliberately not the ordering criterion for this
            // chain; see PLAN.md 34.3.
            //
            // Tier 1: codex-spark at LOW effort -- cheapest per decision, and the
            //         judgement is shallow by construction.
            // Tier 2: gemini flash -- 1M context is the load-bearing property for
            //         duplicate detection against a large open-issue corpus.
            // Tier 3: claude sonnet -- different vendor, so a provider-wide outage or
            //         quota exhaustion cannot take out the whole chain.
            // Timeouts are short: a triage call that needs three minutes has already
            // failed at being cheap, and should fall through rather than block a sweep.
            AgenticStage::IssueTriage => vec![
                ModelExecutionConfig {
                    provider: ModelProvider::OpenAiCodex,
                    specific_model: Some("gpt-5.3-codex-spark".to_string()),
                    reasoning_effort: "low".to_string(),
                    print_timeout_secs: 60,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::Antigravity,
                    specific_model: Some("gemini-3.7-flash".to_string()),
                    reasoning_effort: "low".to_string(),
                    print_timeout_secs: 90,
                },
                ModelExecutionConfig {
                    provider: ModelProvider::AnthropicClaudeCode,
                    specific_model: Some("claude-3-7-sonnet".to_string()),
                    reasoning_effort: "low".to_string(),
                    print_timeout_secs: 90,
                },
            ],
        };

        StageFallbackChain { stage, tiers }
    }

    /// Dispatches prompt across the complete multi-tier fallback chain until success
    pub async fn dispatch_stage(
        &self,
        stage: AgenticStage,
        prompt: &str,
        working_dir: &Path,
    ) -> Result<String> {
        let chain = Self::get_stage_fallback_chain(stage);
        let total_tiers = chain.tiers.len();

        for (idx, tier) in chain.tiers.iter().enumerate() {
            let tier_num = idx + 1;
            info!(
                "🚀 [Stage Pipeline] Stage: {} | Tier {}/{}: {} ({:?}, effort: {})",
                stage.display_name(),
                tier_num,
                total_tiers,
                tier.resolved_model(),
                tier.provider,
                tier.reasoning_effort
            );

            match self
                .executor
                .execute_configured(prompt, working_dir, tier)
                .await
            {
                Ok(result) if !result.trim().is_empty() => {
                    info!(
                        "✅ [Stage Pipeline] Stage: {} succeeded on Tier {} ({})",
                        stage.display_name(),
                        tier_num,
                        tier.resolved_model()
                    );
                    return Ok(result);
                }
                Ok(_) => {
                    warn!(
                        "⚠️ [Stage Pipeline] Tier {} ({}) returned empty output. Progressing to next fallback tier...",
                        tier_num,
                        tier.resolved_model()
                    );
                }
                Err(e) => {
                    warn!(
                        "⚠️ [Stage Pipeline] Tier {} ({}) failed: {}. Progressing to next fallback tier...",
                        tier_num,
                        tier.resolved_model(),
                        e
                    );
                }
            }
        }

        anyhow::bail!(
            "🚨 [Stage Pipeline] All {} fallback tiers exhausted and failed for stage: {}",
            total_tiers,
            stage.display_name()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_fallback_chains_have_multiple_tiers() {
        // Recon has 4 tiers
        let recon = StageModelRouter::get_stage_fallback_chain(AgenticStage::Recon);
        assert_eq!(recon.tiers.len(), 4);
        assert_eq!(
            recon.primary().unwrap().resolved_model(),
            "gpt-5.3-codex-spark"
        );
        assert_eq!(recon.fallbacks()[0].resolved_model(), "gemini-3.7-flash");
        assert_eq!(recon.fallbacks()[1].resolved_model(), "claude-3-7-sonnet");

        // Planning has 4 tiers
        let plan = StageModelRouter::get_stage_fallback_chain(AgenticStage::Planning);
        assert_eq!(plan.tiers.len(), 4);
        assert_eq!(plan.primary().unwrap().resolved_model(), "fable-5");
        assert_eq!(plan.primary().unwrap().reasoning_effort, "xhigh");
        assert_eq!(plan.fallbacks()[0].resolved_model(), "gpt-5.6-sol");
        assert_eq!(plan.fallbacks()[0].reasoning_effort, "xhigh");
        assert_eq!(plan.fallbacks()[1].resolved_model(), "opus-5");

        // Plan Review has 4 tiers
        let plan_rev = StageModelRouter::get_stage_fallback_chain(AgenticStage::PlanReview);
        assert_eq!(plan_rev.tiers.len(), 4);
        assert_eq!(plan_rev.primary().unwrap().resolved_model(), "opus-5");
        assert_eq!(plan_rev.primary().unwrap().reasoning_effort, "high");
        assert_eq!(plan_rev.fallbacks()[0].resolved_model(), "gpt-5.6-sol");

        // Implementation has 4 tiers
        let impl_stage = StageModelRouter::get_stage_fallback_chain(AgenticStage::Implementation);
        assert_eq!(impl_stage.tiers.len(), 4);
        assert_eq!(impl_stage.primary().unwrap().resolved_model(), "grok-4.6");
        assert_eq!(impl_stage.primary().unwrap().reasoning_effort, "xhigh");
        assert_eq!(
            impl_stage.fallbacks()[0].resolved_model(),
            "gemini-3.6-flash"
        );
        assert_eq!(impl_stage.fallbacks()[1].resolved_model(), "gpt-5.6-sol");

        // Code Review & Audit has 4 tiers
        let code_rev = StageModelRouter::get_stage_fallback_chain(AgenticStage::CodeReviewAudit);
        assert_eq!(code_rev.tiers.len(), 4);
        assert_eq!(code_rev.primary().unwrap().resolved_model(), "opus-5");
        assert_eq!(code_rev.fallbacks()[0].resolved_model(), "gpt-5.6-sol");
    }
}
