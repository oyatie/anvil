use anyhow::{bail, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::ai_driver::provider::ModelProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub timestamp: DateTime<Utc>,
    pub model: String,
    pub tokens_consumed: usize,
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone)]
pub struct ManagedAccount {
    pub account_id: String,
    pub provider: ModelProvider,
    pub auth_profile_or_key: Option<String>,
    pub max_5hr_tokens: usize,
    pub max_weekly_budget_usd: f64,
    pub usage_history: VecDeque<UsageRecord>,
    pub cooldown_until: Option<Instant>,
    pub last_leased_at: Instant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountQuotaView {
    pub account_id: String,
    pub provider: String,
    pub used_5hr_tokens: usize,
    pub max_5hr_tokens: usize,
    pub pct_5hr_used: f64,
    pub remaining_5hr_tokens: usize,
    pub weekly_spent_usd: f64,
    pub weekly_budget_usd: f64,
    pub pct_weekly_spent: f64,
    pub is_active: bool,
    pub cooldown_remaining_secs: u64,
}

pub type AccountPoolMap = HashMap<ModelProvider, Vec<Arc<RwLock<ManagedAccount>>>>;

#[derive(Debug, Clone)]
pub struct AccountPoolManager {
    pools: Arc<RwLock<AccountPoolMap>>,
}

impl Default for AccountPoolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountPoolManager {
    pub fn new() -> Self {
        let mut pools = HashMap::new();

        // 1. Anthropic Claude Account Pool (Multi-Account Pooling)
        let claude_accounts = vec![
            Arc::new(RwLock::new(ManagedAccount {
                account_id: "claude-pool-alpha".to_string(),
                provider: ModelProvider::AnthropicClaudeCode,
                auth_profile_or_key: Some("CLAUDE_ACCOUNT_ALPHA".to_string()),
                max_5hr_tokens: 500_000,
                max_weekly_budget_usd: 100.0,
                usage_history: VecDeque::new(),
                cooldown_until: None,
                last_leased_at: Instant::now(),
            })),
            Arc::new(RwLock::new(ManagedAccount {
                account_id: "claude-pool-beta".to_string(),
                provider: ModelProvider::AnthropicClaudeCode,
                auth_profile_or_key: Some("CLAUDE_ACCOUNT_BETA".to_string()),
                max_5hr_tokens: 500_000,
                max_weekly_budget_usd: 100.0,
                usage_history: VecDeque::new(),
                cooldown_until: None,
                last_leased_at: Instant::now(),
            })),
        ];
        pools.insert(ModelProvider::AnthropicClaudeCode, claude_accounts);

        // 2. OpenAI Codex Account Pool (Multi-Account Pooling)
        let codex_accounts = vec![
            Arc::new(RwLock::new(ManagedAccount {
                account_id: "codex-pool-primary".to_string(),
                provider: ModelProvider::OpenAiCodex,
                auth_profile_or_key: Some("OPENAI_API_KEY_PRIMARY".to_string()),
                max_5hr_tokens: 1_000_000,
                max_weekly_budget_usd: 150.0,
                usage_history: VecDeque::new(),
                cooldown_until: None,
                last_leased_at: Instant::now(),
            })),
            Arc::new(RwLock::new(ManagedAccount {
                account_id: "codex-pool-secondary".to_string(),
                provider: ModelProvider::OpenAiCodex,
                auth_profile_or_key: Some("OPENAI_API_KEY_SECONDARY".to_string()),
                max_5hr_tokens: 1_000_000,
                max_weekly_budget_usd: 150.0,
                usage_history: VecDeque::new(),
                cooldown_until: None,
                last_leased_at: Instant::now(),
            })),
        ];
        pools.insert(ModelProvider::OpenAiCodex, codex_accounts);

        // 3. Antigravity / Gemini Account Pool
        let agy_accounts = vec![
            Arc::new(RwLock::new(ManagedAccount {
                account_id: "agy-pool-tier0".to_string(),
                provider: ModelProvider::Antigravity,
                auth_profile_or_key: Some("AGY_AUTH_PROFILE_DEFAULT".to_string()),
                max_5hr_tokens: 2_000_000,
                max_weekly_budget_usd: 200.0,
                usage_history: VecDeque::new(),
                cooldown_until: None,
                last_leased_at: Instant::now(),
            })),
            Arc::new(RwLock::new(ManagedAccount {
                account_id: "agy-pool-backup".to_string(),
                provider: ModelProvider::Antigravity,
                auth_profile_or_key: Some("AGY_AUTH_PROFILE_BACKUP".to_string()),
                max_5hr_tokens: 2_000_000,
                max_weekly_budget_usd: 200.0,
                usage_history: VecDeque::new(),
                cooldown_until: None,
                last_leased_at: Instant::now(),
            })),
        ];
        pools.insert(ModelProvider::Antigravity, agy_accounts);

        Self {
            pools: Arc::new(RwLock::new(pools)),
        }
    }

    /// Leases the healthiest, least-loaded account in the provider's pool
    pub async fn lease_account(
        &self,
        provider: ModelProvider,
    ) -> Result<Arc<RwLock<ManagedAccount>>> {
        let guard = self.pools.read().await;
        let accounts = guard.get(&provider).ok_or_else(|| {
            anyhow::anyhow!("No account pool configured for provider {:?}", provider)
        })?;

        if accounts.is_empty() {
            bail!("Account pool for provider {:?} is empty", provider);
        }

        let now = Instant::now();
        let utc_now = Utc::now();
        let five_hours_ago = utc_now - ChronoDuration::hours(5);

        let mut best_account: Option<(Arc<RwLock<ManagedAccount>>, usize, Instant)> = None;

        for acc_arc in accounts {
            let mut acc = acc_arc.write().await;

            // Check if cooldown expired
            if let Some(cooldown) = acc.cooldown_until {
                if now >= cooldown {
                    acc.cooldown_until = None;
                    info!(
                        "Account {} cooldown expired. Re-enabled in pool.",
                        acc.account_id
                    );
                } else {
                    continue; // Skip accounts currently in cooldown
                }
            }

            // Prune usage history older than 7 days
            let seven_days_ago = utc_now - ChronoDuration::days(7);
            acc.usage_history.retain(|r| r.timestamp >= seven_days_ago);

            // Compute current 5-hour rolling tokens
            let tokens_5hr: usize = acc
                .usage_history
                .iter()
                .filter(|r| r.timestamp >= five_hours_ago)
                .map(|r| r.tokens_consumed)
                .sum();

            // Check if 5-hour quota exceeded
            if tokens_5hr >= acc.max_5hr_tokens {
                warn!(
                    "Account {} reached 5-hour token ceiling ({}/{} tokens). Skipping.",
                    acc.account_id, tokens_5hr, acc.max_5hr_tokens
                );
                continue;
            }

            // Pick account with lowest 5-hour load, breaking ties with least recently leased
            let is_better = match &best_account {
                None => true,
                Some((_, best_tokens, best_time)) => {
                    tokens_5hr < *best_tokens
                        || (tokens_5hr == *best_tokens && acc.last_leased_at < *best_time)
                }
            };

            if is_better {
                best_account = Some((Arc::clone(acc_arc), tokens_5hr, acc.last_leased_at));
            }
        }

        if let Some((selected_acc, _, _)) = best_account {
            let mut acc = selected_acc.write().await;
            acc.last_leased_at = Instant::now();
            Ok(Arc::clone(&selected_acc))
        } else {
            bail!(
                "All accounts in pool for provider {:?} are currently rate-limited or quota-exhausted",
                provider
            )
        }
    }

    /// Records token spend and cost into an account's sliding usage history
    pub async fn record_spend(
        &self,
        account_id: &str,
        model: &str,
        tokens: usize,
        cost_usd: f64,
    ) -> Result<AccountQuotaView> {
        let guard = self.pools.read().await;

        for accounts in guard.values() {
            for acc_arc in accounts {
                let mut acc = acc_arc.write().await;
                if acc.account_id == account_id {
                    let now = Utc::now();
                    acc.usage_history.push_back(UsageRecord {
                        timestamp: now,
                        model: model.to_string(),
                        tokens_consumed: tokens,
                        estimated_cost_usd: cost_usd,
                    });

                    let five_hours_ago = now - ChronoDuration::hours(5);
                    let seven_days_ago = now - ChronoDuration::days(7);

                    acc.usage_history.retain(|r| r.timestamp >= seven_days_ago);

                    let used_5hr: usize = acc
                        .usage_history
                        .iter()
                        .filter(|r| r.timestamp >= five_hours_ago)
                        .map(|r| r.tokens_consumed)
                        .sum();

                    let spent_weekly: f64 = acc
                        .usage_history
                        .iter()
                        .filter(|r| r.timestamp >= seven_days_ago)
                        .map(|r| r.estimated_cost_usd)
                        .sum();

                    let pct_5hr = (used_5hr as f64 / acc.max_5hr_tokens.max(1) as f64) * 100.0;
                    let pct_weekly = (spent_weekly / acc.max_weekly_budget_usd.max(0.01)) * 100.0;
                    let remaining_5hr = acc.max_5hr_tokens.saturating_sub(used_5hr);

                    let cooldown_secs = acc
                        .cooldown_until
                        .map(|c| c.saturating_duration_since(Instant::now()).as_secs())
                        .unwrap_or(0);

                    return Ok(AccountQuotaView {
                        account_id: acc.account_id.clone(),
                        provider: acc.provider.display_name().to_string(),
                        used_5hr_tokens: used_5hr,
                        max_5hr_tokens: acc.max_5hr_tokens,
                        pct_5hr_used: pct_5hr,
                        remaining_5hr_tokens: remaining_5hr,
                        weekly_spent_usd: spent_weekly,
                        weekly_budget_usd: acc.max_weekly_budget_usd,
                        pct_weekly_spent: pct_weekly,
                        is_active: acc.cooldown_until.is_none() && used_5hr < acc.max_5hr_tokens,
                        cooldown_remaining_secs: cooldown_secs,
                    });
                }
            }
        }

        bail!("Account {} not found in any pool", account_id)
    }

    /// Puts an account into temporary rate limit cooldown
    pub async fn mark_rate_limited(&self, account_id: &str, cooldown: Duration) {
        let guard = self.pools.read().await;
        for accounts in guard.values() {
            for acc_arc in accounts {
                let mut acc = acc_arc.write().await;
                if acc.account_id == account_id {
                    let until = Instant::now() + cooldown;
                    acc.cooldown_until = Some(until);
                    warn!(
                        "🚫 [Rate Limit Cooldown] Marked account {} in cooldown for {}s",
                        account_id,
                        cooldown.as_secs()
                    );
                    return;
                }
            }
        }
    }

    /// Produces full per-account quota and status views for the dashboard and API
    pub async fn get_pool_status_views(&self) -> Vec<AccountQuotaView> {
        let guard = self.pools.read().await;
        let mut views = Vec::new();
        let utc_now = Utc::now();
        let five_hours_ago = utc_now - ChronoDuration::hours(5);
        let seven_days_ago = utc_now - ChronoDuration::days(7);
        let now = Instant::now();

        for accounts in guard.values() {
            for acc_arc in accounts {
                let acc = acc_arc.read().await;

                let used_5hr: usize = acc
                    .usage_history
                    .iter()
                    .filter(|r| r.timestamp >= five_hours_ago)
                    .map(|r| r.tokens_consumed)
                    .sum();

                let spent_weekly: f64 = acc
                    .usage_history
                    .iter()
                    .filter(|r| r.timestamp >= seven_days_ago)
                    .map(|r| r.estimated_cost_usd)
                    .sum();

                let pct_5hr = (used_5hr as f64 / acc.max_5hr_tokens.max(1) as f64) * 100.0;
                let pct_weekly = (spent_weekly / acc.max_weekly_budget_usd.max(0.01)) * 100.0;
                let remaining_5hr = acc.max_5hr_tokens.saturating_sub(used_5hr);

                let cooldown_secs = acc
                    .cooldown_until
                    .map(|c| c.saturating_duration_since(now).as_secs())
                    .unwrap_or(0);

                views.push(AccountQuotaView {
                    account_id: acc.account_id.clone(),
                    provider: acc.provider.display_name().to_string(),
                    used_5hr_tokens: used_5hr,
                    max_5hr_tokens: acc.max_5hr_tokens,
                    pct_5hr_used: pct_5hr,
                    remaining_5hr_tokens: remaining_5hr,
                    weekly_spent_usd: spent_weekly,
                    weekly_budget_usd: acc.max_weekly_budget_usd,
                    pct_weekly_spent: pct_weekly,
                    is_active: acc.cooldown_until.is_none() && used_5hr < acc.max_5hr_tokens,
                    cooldown_remaining_secs: cooldown_secs,
                });
            }
        }

        views
    }
}
