use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::{error, warn};

use super::account_pool::AccountPoolManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaBudgetReport {
    pub total_tokens_consumed: u64,
    pub estimated_cost_usd: f64,
    pub max_cost_budget_usd: f64,
    pub is_circuit_broken: bool,
}

#[derive(Clone)]
pub struct QuotaEnforcer {
    total_tokens: Arc<AtomicU64>,
    total_cost_micro_usd: Arc<AtomicU64>, // cost in millionths of a dollar ($1 = 1,000,000)
    max_cost_budget_usd: f64,
    default_cost_per_million_tokens: f64,
    pub account_pool: AccountPoolManager,
}

impl Default for QuotaEnforcer {
    fn default() -> Self {
        Self::new(100.0, 3.0) // Default $100 total budget, $3/1M tokens baseline
    }
}

impl QuotaEnforcer {
    pub fn new(max_cost_budget_usd: f64, default_cost_per_million_tokens: f64) -> Self {
        Self {
            total_tokens: Arc::new(AtomicU64::new(0)),
            total_cost_micro_usd: Arc::new(AtomicU64::new(0)),
            max_cost_budget_usd,
            default_cost_per_million_tokens,
            account_pool: AccountPoolManager::new(),
        }
    }

    /// Computes model-specific pricing per 1M tokens based on provider pricing tiers
    pub fn model_rate_per_million(&self, model: &str) -> f64 {
        let lower = model.to_lowercase();
        if lower.contains("opus") {
            30.0 // Claude Opus 5 average blended price
        } else if lower.contains("gpt-5") || lower.contains("gpt-5.6") || lower.contains("o3") {
            15.0 // GPT-5.6-Sol / O3 high reasoning
        } else if lower.contains("sonnet") || lower.contains("claude-3-7") {
            8.0 // Claude 3.7 Sonnet
        } else if lower.contains("flash") || lower.contains("gemini") {
            1.5 // Gemini 3.7 Flash High
        } else {
            self.default_cost_per_million_tokens // Configured default baseline
        }
    }

    /// Records model-specific token consumption and verifies quota budget limits
    pub fn record_model_spend(&self, model: &str, tokens: usize) -> Result<QuotaBudgetReport> {
        let rate = self.model_rate_per_million(model);
        let cost_usd = (tokens as f64 / 1_000_000.0) * rate;
        let micro_usd = (cost_usd * 1_000_000.0) as u64;

        let prev_tokens = self
            .total_tokens
            .fetch_add(tokens as u64, Ordering::Relaxed);
        let total_tokens = prev_tokens + tokens as u64;

        let prev_micro = self
            .total_cost_micro_usd
            .fetch_add(micro_usd, Ordering::Relaxed);
        let total_micro = prev_micro + micro_usd;
        let total_cost_usd = total_micro as f64 / 1_000_000.0;

        let is_circuit_broken = total_cost_usd > self.max_cost_budget_usd;

        if is_circuit_broken {
            error!(
                "🚨 [QUOTA CIRCUIT BREAKER TRIPPED] LLM spend (${:.2}) exceeded max cluster budget (${:.2}). Total tokens: {}",
                total_cost_usd, self.max_cost_budget_usd, total_tokens
            );
            bail!(
                "Quota budget exceeded: spend of ${:.2} breached limit of ${:.2}",
                total_cost_usd,
                self.max_cost_budget_usd
            );
        } else if total_cost_usd > self.max_cost_budget_usd * 0.8 {
            warn!(
                "⚠️ [Quota Warning] LLM token budget utilization at {:.1}% (${:.2} / ${:.2})",
                (total_cost_usd / self.max_cost_budget_usd) * 100.0,
                total_cost_usd,
                self.max_cost_budget_usd
            );
        }

        Ok(QuotaBudgetReport {
            total_tokens_consumed: total_tokens,
            estimated_cost_usd: total_cost_usd,
            max_cost_budget_usd: self.max_cost_budget_usd,
            is_circuit_broken,
        })
    }

    /// Records token consumption at default rate
    pub fn record_and_verify_token_spend(&self, tokens: usize) -> Result<QuotaBudgetReport> {
        self.record_model_spend("default", tokens)
    }

    pub fn current_spend_usd(&self) -> f64 {
        let micro = self.total_cost_micro_usd.load(Ordering::Relaxed);
        micro as f64 / 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_multi_model_rates() {
        let enforcer = QuotaEnforcer::new(50.0, 3.0);

        // Record 1M tokens of Opus ($30)
        let rep1 = enforcer
            .record_model_spend("claude-opus-5", 1_000_000)
            .unwrap();
        assert_eq!(rep1.estimated_cost_usd, 30.0);
        assert!(!rep1.is_circuit_broken);

        // Record 1M tokens of Gemini Flash ($1.50)
        let rep2 = enforcer
            .record_model_spend("gemini-3.7-flash", 1_000_000)
            .unwrap();
        assert_eq!(rep2.estimated_cost_usd, 31.50);

        // Record 1M tokens of GPT-5.6 ($15) -> Total $46.50
        let rep3 = enforcer
            .record_model_spend("gpt-5.6-sol", 1_000_000)
            .unwrap();
        assert_eq!(rep3.estimated_cost_usd, 46.50);

        // Next 1M tokens of Opus ($30) breaches $50 limit
        assert!(
            enforcer
                .record_model_spend("claude-opus-5", 1_000_000)
                .is_err()
        );
    }
}
