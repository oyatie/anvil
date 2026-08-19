use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{error, warn};

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
    max_cost_budget_usd: f64,
    cost_per_million_tokens: f64,
}

impl Default for QuotaEnforcer {
    fn default() -> Self {
        Self::new(100.0, 3.0) // Default $100 total budget, $3/1M tokens average
    }
}

impl QuotaEnforcer {
    pub fn new(max_cost_budget_usd: f64, cost_per_million_tokens: f64) -> Self {
        Self {
            total_tokens: Arc::new(AtomicU64::new(0)),
            max_cost_budget_usd,
            cost_per_million_tokens,
        }
    }

    /// Records token consumption and verifies quota budget limits
    pub fn record_and_verify_token_spend(&self, tokens: usize) -> Result<QuotaBudgetReport> {
        let prev = self
            .total_tokens
            .fetch_add(tokens as u64, Ordering::Relaxed);
        let total = prev + tokens as u64;
        let estimated_cost = (total as f64 / 1_000_000.0) * self.cost_per_million_tokens;

        let is_circuit_broken = estimated_cost > self.max_cost_budget_usd;

        if is_circuit_broken {
            error!(
                "🚨 [QUOTA CIRCUIT BREAKER TRIPPED] Estimated LLM spend (${:.2}) exceeded maximum budget (${:.2}). Total tokens: {}",
                estimated_cost, self.max_cost_budget_usd, total
            );
            bail!(
                "Quota budget exceeded: spend of ${:.2} breached limit of ${:.2}",
                estimated_cost,
                self.max_cost_budget_usd
            );
        } else if estimated_cost > self.max_cost_budget_usd * 0.8 {
            warn!(
                "⚠️ [Quota Warning] LLM token budget utilization at {:.1}% (${:.2} / ${:.2})",
                (estimated_cost / self.max_cost_budget_usd) * 100.0,
                estimated_cost,
                self.max_cost_budget_usd
            );
        }

        Ok(QuotaBudgetReport {
            total_tokens_consumed: total,
            estimated_cost_usd: estimated_cost,
            max_cost_budget_usd: self.max_cost_budget_usd,
            is_circuit_broken,
        })
    }

    pub fn current_spend_usd(&self) -> f64 {
        let total = self.total_tokens.load(Ordering::Relaxed);
        (total as f64 / 1_000_000.0) * self.cost_per_million_tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quota_enforcer_circuit_breaking() {
        let enforcer = QuotaEnforcer::new(10.0, 5.0); // $10 max budget at $5/M tokens (max 2M tokens)

        // 1M tokens = $5 (within budget)
        let rep1 = enforcer.record_and_verify_token_spend(1_000_000);
        assert!(rep1.is_ok());
        assert_eq!(rep1.unwrap().estimated_cost_usd, 5.0);

        // Another 1.5M tokens = $12.50 (exceeds $10 budget)
        let rep2 = enforcer.record_and_verify_token_spend(1_500_000);
        assert!(rep2.is_err());
    }
}
