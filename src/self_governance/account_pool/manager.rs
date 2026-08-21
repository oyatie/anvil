use anyhow::{Result, bail};
use chrono::{Duration as ChronoDuration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::defaults::bootstrap_default_accounts;
use super::quota_view::compute_quota_view;
use super::types::{
    AccountPoolMap, AccountQuotaView, AffinityCacheMap, ManagedAccount, UsageRecord,
};
use crate::ai_driver::provider::ModelProvider;

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
        for (provider, accounts) in bootstrap_default_accounts() {
            pools.insert(provider, accounts);
        }

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
            if let Some((acc_id, expires_at)) = cache_guard.get(key)
                && now < *expires_at
            {
                let pool_guard = self.pools.read().await;
                if let Some(accounts) = pool_guard.get(&provider) {
                    for acc_arc in accounts {
                        let mut acc = acc_arc.write().await;
                        if acc.account_id == *acc_id
                            && !acc.is_draining
                            && (acc.cooldown_until.is_none() || now >= acc.cooldown_until.unwrap())
                        {
                            let used_5hr: usize = acc
                                .usage_history
                                .iter()
                                .filter(|r| r.timestamp >= five_hours_ago)
                                .map(|r| r.tokens_consumed)
                                .sum();

                            let has_headroom = match acc.max_5hr_tokens {
                                Some(max) => used_5hr < max,
                                None => true,
                            };

                            if has_headroom {
                                acc.last_leased_at = now;
                                cache_guard.insert(
                                    key.to_string(),
                                    (acc.account_id.clone(), now + Duration::from_secs(300)),
                                );
                                info!(
                                    "⚡ [Prompt Cache Hit] Routed to warm affinity '{}' for '{}'",
                                    acc.account_id, key
                                );
                                return Ok(Arc::clone(acc_arc));
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

            if acc.is_draining {
                continue;
            }

            if let Some(cooldown) = acc.cooldown_until {
                if now >= cooldown {
                    acc.cooldown_until = None;
                    info!("Account {} cooldown expired. Re-enabled.", acc.account_id);
                } else {
                    continue;
                }
            }

            let seven_days_ago = utc_now - ChronoDuration::days(7);
            acc.usage_history.retain(|r| r.timestamp >= seven_days_ago);

            let tokens_5hr: usize = acc
                .usage_history
                .iter()
                .filter(|r| r.timestamp >= five_hours_ago)
                .map(|r| r.tokens_consumed)
                .sum();

            if let Some(max) = acc.max_5hr_tokens
                && tokens_5hr >= max
            {
                warn!(
                    "Account {} reached 5hr ceiling ({}/{}). Skipping.",
                    acc.account_id, tokens_5hr, max
                );
                continue;
            }

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
                "All accounts for {:?} are rate-limited, draining, or quota-exhausted",
                provider
            )
        }
    }

    /// Records token spend and cost, returning updated quota view
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

                    let seven_days_ago = now - ChronoDuration::days(7);
                    acc.usage_history.retain(|r| r.timestamp >= seven_days_ago);

                    return Ok(compute_quota_view(&acc));
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
                        "🚫 [Rate Limit] Account {} in cooldown for {}s",
                        account_id,
                        cooldown.as_secs()
                    );
                    return;
                }
            }
        }
    }

    /// Produces full per-account quota and status views for dashboard and API
    pub async fn get_pool_status_views(&self) -> Vec<AccountQuotaView> {
        let guard = self.pools.read().await;
        let mut views = Vec::new();

        for accounts in guard.values() {
            for acc_arc in accounts {
                let acc = acc_arc.read().await;
                views.push(compute_quota_view(&acc));
            }
        }

        views
    }
}
