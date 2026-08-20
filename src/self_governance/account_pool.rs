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
    pub max_5hr_tokens: Option<usize>,
    pub max_weekly_budget_usd: Option<f64>,
    pub usage_history: VecDeque<UsageRecord>,
    pub cooldown_until: Option<Instant>,
    pub last_leased_at: Instant,
    pub is_draining: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountQuotaView {
    pub account_id: String,
    pub provider: String,
    pub used_5hr_tokens: usize,
    pub max_5hr_tokens: Option<usize>,
    pub remaining_5hr_tokens: Option<usize>,
    pub pct_5hr_used: Option<f64>,
    pub weekly_spent_usd: f64,
    pub weekly_budget_usd: Option<f64>,
    pub pct_weekly_spent: Option<f64>,
    pub quota_description: String,
    pub is_active: bool,
    pub is_draining: bool,
    pub lifecycle_state: String,
    pub cooldown_remaining_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddAccountPayload {
    pub account_id: String,
    pub provider: String,
    pub auth_profile_or_key: Option<String>,
    pub max_5hr_tokens: Option<usize>,
    pub max_weekly_budget_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrainAccountPayload {
    pub account_id: String,
}

pub type AccountPoolMap = HashMap<ModelProvider, Vec<Arc<RwLock<ManagedAccount>>>>;
pub type AffinityCacheMap = HashMap<String, (String, Instant)>; // affinity_key -> (account_id, expires_at)

#[derive(Debug, Clone)]
pub struct AccountPoolManager {
    pools: Arc<RwLock<AccountPoolMap>>,
    affinity_cache: Arc<RwLock<AffinityCacheMap>>,
}

impl Default for AccountPoolManager {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountPoolManager {
    pub fn new() -> Self {
        let mut pools = HashMap::new();

        // Authentic local CLI subscription accounts discovered from host environment
        // 1. Anthropic Claude Code (Local logged-in CLI: ~/.claude)
        let claude_accounts = vec![Arc::new(RwLock::new(ManagedAccount {
            account_id: "claude:cli-default".to_string(),
            provider: ModelProvider::AnthropicClaudeCode,
            auth_profile_or_key: Some("HOST_CLAUDE_CLI_AUTH".to_string()),
            max_5hr_tokens: None,        // Uncapped CLI subscription
            max_weekly_budget_usd: None, // Uncapped CLI subscription
            usage_history: VecDeque::new(),
            cooldown_until: None,
            last_leased_at: Instant::now(),
            is_draining: false,
        }))];
        pools.insert(ModelProvider::AnthropicClaudeCode, claude_accounts);

        // 2. OpenAI Codex (Local logged-in CLI: ~/.codex)
        let codex_accounts = vec![Arc::new(RwLock::new(ManagedAccount {
            account_id: "codex:cli-default".to_string(),
            provider: ModelProvider::OpenAiCodex,
            auth_profile_or_key: Some("HOST_CODEX_CLI_AUTH".to_string()),
            max_5hr_tokens: None,
            max_weekly_budget_usd: None,
            usage_history: VecDeque::new(),
            cooldown_until: None,
            last_leased_at: Instant::now(),
            is_draining: false,
        }))];
        pools.insert(ModelProvider::OpenAiCodex, codex_accounts);

        // 3. Google Antigravity / Gemini (Local logged-in CLI: agy)
        let agy_accounts = vec![Arc::new(RwLock::new(ManagedAccount {
            account_id: "agy:cli-default".to_string(),
            provider: ModelProvider::Antigravity,
            auth_profile_or_key: Some("HOST_AGY_CLI_AUTH".to_string()),
            max_5hr_tokens: None,
            max_weekly_budget_usd: None,
            usage_history: VecDeque::new(),
            cooldown_until: None,
            last_leased_at: Instant::now(),
            is_draining: false,
        }))];
        pools.insert(ModelProvider::Antigravity, agy_accounts);

        // 4. Cursor Agent (Local logged-in CLI: ~/.cursor)
        let cursor_accounts = vec![Arc::new(RwLock::new(ManagedAccount {
            account_id: "cursor:cli-default".to_string(),
            provider: ModelProvider::CursorAgent,
            auth_profile_or_key: Some("HOST_CURSOR_CLI_AUTH".to_string()),
            max_5hr_tokens: None,
            max_weekly_budget_usd: None,
            usage_history: VecDeque::new(),
            cooldown_until: None,
            last_leased_at: Instant::now(),
            is_draining: false,
        }))];
        pools.insert(ModelProvider::CursorAgent, cursor_accounts);

        // 5. xAI Grok (Local logged-in CLI: ~/.grok)
        let grok_accounts = vec![Arc::new(RwLock::new(ManagedAccount {
            account_id: "grok:cli-default".to_string(),
            provider: ModelProvider::XAiGrok,
            auth_profile_or_key: Some("HOST_GROK_CLI_AUTH".to_string()),
            max_5hr_tokens: None,
            max_weekly_budget_usd: None,
            usage_history: VecDeque::new(),
            cooldown_until: None,
            last_leased_at: Instant::now(),
            is_draining: false,
        }))];
        pools.insert(ModelProvider::XAiGrok, grok_accounts);

        Self {
            pools: Arc::new(RwLock::new(pools)),
            affinity_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Dynamically registers a new managed account into the pool
    pub async fn add_account(&self, account: ManagedAccount) -> Result<()> {
        let provider = account.provider.clone();
        let account_id = account.account_id.clone();
        let mut guard = self.pools.write().await;

        let entry = guard.entry(provider).or_default();
        // Check if account_id already exists
        for existing in entry.iter() {
            if existing.read().await.account_id == account_id {
                bail!("Account ID '{}' already exists in pool", account_id);
            }
        }

        info!(
            "➕ [Account Pool] Registered new account '{}' for provider {:?}",
            account_id, account.provider
        );
        entry.push(Arc::new(RwLock::new(account)));
        Ok(())
    }

    /// Marks an account into Graceful Draining mode
    pub async fn drain_account(&self, account_id: &str) -> Result<()> {
        let guard = self.pools.read().await;
        for accounts in guard.values() {
            for acc_arc in accounts {
                let mut acc = acc_arc.write().await;
                if acc.account_id == account_id {
                    acc.is_draining = true;
                    info!(
                        "👋 [Account Pool] Account '{}' marked as DRAINING.",
                        account_id
                    );
                    return Ok(());
                }
            }
        }
        bail!("Account '{}' not found in any pool", account_id)
    }

    /// Resumes a drained account back into Active service
    pub async fn resume_account(&self, account_id: &str) -> Result<()> {
        let guard = self.pools.read().await;
        for accounts in guard.values() {
            for acc_arc in accounts {
                let mut acc = acc_arc.write().await;
                if acc.account_id == account_id {
                    acc.is_draining = false;
                    acc.cooldown_until = None;
                    info!(
                        "🟢 [Account Pool] Account '{}' resumed to ACTIVE.",
                        account_id
                    );
                    return Ok(());
                }
            }
        }
        bail!("Account '{}' not found in any pool", account_id)
    }

    /// Leases an account with context-aware prompt-cache affinity
    pub async fn lease_account_with_affinity(
        &self,
        provider: ModelProvider,
        context_affinity_key: Option<&str>,
    ) -> Result<Arc<RwLock<ManagedAccount>>> {
        let now = Instant::now();
        let utc_now = Utc::now();
        let five_hours_ago = utc_now - ChronoDuration::hours(5);

        // 1. Check prompt-cache affinity table
        if let Some(key) = context_affinity_key {
            let mut cache_guard = self.affinity_cache.write().await;
            if let Some((acc_id, expires_at)) = cache_guard.get(key) {
                if now < *expires_at {
                    // Try to find this exact account
                    let pool_guard = self.pools.read().await;
                    if let Some(accounts) = pool_guard.get(&provider) {
                        for acc_arc in accounts {
                            let mut acc = acc_arc.write().await;
                            if acc.account_id == *acc_id
                                && !acc.is_draining
                                && (acc.cooldown_until.is_none()
                                    || now >= acc.cooldown_until.unwrap())
                            {
                                let used_5hr: usize = acc
                                    .usage_history
                                    .iter()
                                    .filter(|r| r.timestamp >= five_hours_ago)
                                    .map(|r| r.tokens_consumed)
                                    .sum();

                                let has_headroom = match acc.max_5hr_tokens {
                                    Some(max) => used_5hr < max,
                                    None => true, // Uncapped subscription
                                };

                                if has_headroom {
                                    acc.last_leased_at = now;
                                    // Extend prompt-cache affinity TTL by 5 minutes
                                    cache_guard.insert(
                                        key.to_string(),
                                        (acc.account_id.clone(), now + Duration::from_secs(300)),
                                    );
                                    info!(
                                        "⚡ [Prompt Cache Hit] Routed request to warm affinity account '{}' for key '{}'",
                                        acc.account_id, key
                                    );
                                    return Ok(Arc::clone(acc_arc));
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Standard Least-Loaded Leasing
        let leased = self.lease_account(provider).await?;

        // 3. Record new affinity mapping if key provided
        if let Some(key) = context_affinity_key {
            let acc_id = leased.read().await.account_id.clone();
            let mut cache_guard = self.affinity_cache.write().await;
            cache_guard.insert(key.to_string(), (acc_id, now + Duration::from_secs(300)));
        }

        Ok(leased)
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

            // Skip draining accounts
            if acc.is_draining {
                continue;
            }

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

            // Check if 5-hour quota exceeded (if configured)
            if let Some(max) = acc.max_5hr_tokens {
                if tokens_5hr >= max {
                    warn!(
                        "Account {} reached 5-hour token ceiling ({}/{} tokens). Skipping.",
                        acc.account_id, tokens_5hr, max
                    );
                    continue;
                }
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
                "All accounts in pool for provider {:?} are currently rate-limited, draining, or quota-exhausted",
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

                    let spent_clean = spent_weekly.abs().max(0.0);

                    let (pct_5hr, rem_5hr) = match acc.max_5hr_tokens {
                        Some(max) => (
                            Some((used_5hr as f64 / max.max(1) as f64) * 100.0),
                            Some(max.saturating_sub(used_5hr)),
                        ),
                        None => (None, None),
                    };

                    let pct_weekly = acc
                        .max_weekly_budget_usd
                        .map(|b| (spent_clean / b.max(0.01)) * 100.0);

                    let quota_desc = match (acc.max_5hr_tokens, acc.max_weekly_budget_usd) {
                        (Some(max), Some(b)) => {
                            format!("{:.0}k 5hr / ${:.0} wk", max as f64 / 1000.0, b)
                        }
                        (Some(max), None) => format!("{:.0}k 5hr / Uncapped", max as f64 / 1000.0),
                        (None, Some(b)) => format!("Uncapped 5hr / ${:.0} wk", b),
                        (None, None) => "Dynamic CLI Subscription (Uncapped)".to_string(),
                    };

                    let cooldown_secs = acc
                        .cooldown_until
                        .map(|c| c.saturating_duration_since(Instant::now()).as_secs())
                        .unwrap_or(0);

                    let is_exhausted = acc
                        .max_5hr_tokens
                        .map(|max| used_5hr >= max)
                        .unwrap_or(false);

                    let state_str = if acc.is_draining {
                        "DRAINING".to_string()
                    } else if cooldown_secs > 0 {
                        format!("COOLDOWN ({}s)", cooldown_secs)
                    } else if is_exhausted {
                        "QUOTA_EXHAUSTED".to_string()
                    } else {
                        "ACTIVE".to_string()
                    };

                    return Ok(AccountQuotaView {
                        account_id: acc.account_id.clone(),
                        provider: acc.provider.display_name().to_string(),
                        used_5hr_tokens: used_5hr,
                        max_5hr_tokens: acc.max_5hr_tokens,
                        remaining_5hr_tokens: rem_5hr,
                        pct_5hr_used: pct_5hr,
                        weekly_spent_usd: spent_clean,
                        weekly_budget_usd: acc.max_weekly_budget_usd,
                        pct_weekly_spent: pct_weekly,
                        quota_description: quota_desc,
                        is_active: !acc.is_draining && acc.cooldown_until.is_none() && !is_exhausted,
                        is_draining: acc.is_draining,
                        lifecycle_state: state_str,
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

                let spent_clean = spent_weekly.abs().max(0.0);

                let (pct_5hr, rem_5hr) = match acc.max_5hr_tokens {
                    Some(max) => (
                        Some((used_5hr as f64 / max.max(1) as f64) * 100.0),
                        Some(max.saturating_sub(used_5hr)),
                    ),
                    None => (None, None),
                };

                let pct_weekly = acc
                    .max_weekly_budget_usd
                    .map(|b| (spent_clean / b.max(0.01)) * 100.0);

                let quota_desc = match (acc.max_5hr_tokens, acc.max_weekly_budget_usd) {
                    (Some(max), Some(b)) => {
                        format!("{:.0}k 5hr / ${:.0} wk", max as f64 / 1000.0, b)
                    }
                    (Some(max), None) => format!("{:.0}k 5hr / Uncapped", max as f64 / 1000.0),
                    (None, Some(b)) => format!("Uncapped 5hr / ${:.0} wk", b),
                    (None, None) => "Dynamic CLI Subscription (Uncapped)".to_string(),
                };

                let cooldown_secs = acc
                    .cooldown_until
                    .map(|c| c.saturating_duration_since(now).as_secs())
                    .unwrap_or(0);

                let is_exhausted = acc
                    .max_5hr_tokens
                    .map(|max| used_5hr >= max)
                    .unwrap_or(false);

                let state_str = if acc.is_draining {
                    "DRAINING".to_string()
                } else if cooldown_secs > 0 {
                    format!("COOLDOWN ({}s)", cooldown_secs)
                } else if is_exhausted {
                    "QUOTA_EXHAUSTED".to_string()
                } else {
                    "ACTIVE".to_string()
                };

                views.push(AccountQuotaView {
                    account_id: acc.account_id.clone(),
                    provider: acc.provider.display_name().to_string(),
                    used_5hr_tokens: used_5hr,
                    max_5hr_tokens: acc.max_5hr_tokens,
                    remaining_5hr_tokens: rem_5hr,
                    pct_5hr_used: pct_5hr,
                    weekly_spent_usd: spent_clean,
                    weekly_budget_usd: acc.max_weekly_budget_usd,
                    pct_weekly_spent: pct_weekly,
                    quota_description: quota_desc,
                    is_active: !acc.is_draining && acc.cooldown_until.is_none() && !is_exhausted,
                    is_draining: acc.is_draining,
                    lifecycle_state: state_str,
                    cooldown_remaining_secs: cooldown_secs,
                });
            }
        }

        views
    }
}
