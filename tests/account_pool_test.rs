use anvil::ai_driver::provider::ModelProvider;
use anvil::self_governance::account_pool::{AccountPoolManager, ManagedAccount};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[tokio::test]
async fn test_multi_account_pool_least_loaded_leasing() {
    let pool = AccountPoolManager::new();

    // 1. Lease default discovered Claude CLI account
    let acc1 = pool
        .lease_account(ModelProvider::AnthropicClaudeCode)
        .await
        .unwrap();
    let acc1_id = acc1.read().await.account_id.clone();
    assert_eq!(acc1_id, "claude:cli-default");

    // 2. Add second Claude account with explicit quota and OAuth token
    let acc2_custom = ManagedAccount {
        account_id: "claude-custom-pool-02".to_string(),
        provider: ModelProvider::AnthropicClaudeCode,
        auth_type: anvil::self_governance::AuthType::OAuthToken,
        auth_profile_or_key: Some("CLAUDE_CUSTOM_AUTH".to_string()),
        oauth_token: Some("sk-ant-oat01-test-token-123".to_string()),
        config_dir: None,
        max_5hr_tokens: Some(500_000),
        max_weekly_budget_usd: Some(100.0),
        usage_history: VecDeque::new(),
        cooldown_until: None,
        last_leased_at: Instant::now(),
        is_draining: false,
    };
    pool.add_account(acc2_custom).await.unwrap();

    // 3. Record 100k tokens on default ($3.00)
    let quota1 = pool
        .record_spend(&acc1_id, "claude-opus-5", 100_000, 3.0)
        .await
        .unwrap();
    assert_eq!(quota1.used_5hr_tokens, 100_000);

    // 4. Lease next Claude account -> should pick claude-custom-pool-02 (0 tokens used)
    let acc2 = pool
        .lease_account(ModelProvider::AnthropicClaudeCode)
        .await
        .unwrap();
    let acc2_id = acc2.read().await.account_id.clone();
    assert_eq!(acc2_id, "claude-custom-pool-02");
}

#[tokio::test]
async fn test_multi_account_rate_limit_failover() {
    let pool = AccountPoolManager::new();

    // Add a secondary account to test failover
    let secondary = ManagedAccount {
        account_id: "claude:backup-account".to_string(),
        provider: ModelProvider::AnthropicClaudeCode,
        auth_type: anvil::self_governance::AuthType::ApiKey,
        auth_profile_or_key: Some("CLAUDE_BACKUP_KEY".to_string()),
        oauth_token: None,
        config_dir: None,
        max_5hr_tokens: Some(500_000),
        max_weekly_budget_usd: Some(100.0),
        usage_history: VecDeque::new(),
        cooldown_until: None,
        last_leased_at: Instant::now(),
        is_draining: false,
    };
    pool.add_account(secondary).await.unwrap();

    // Mark default in cooldown
    pool.mark_rate_limited("claude:cli-default", Duration::from_secs(300))
        .await;

    // Lease account -> default skipped, backup leased
    let acc = pool
        .lease_account(ModelProvider::AnthropicClaudeCode)
        .await
        .unwrap();
    assert_eq!(acc.read().await.account_id, "claude:backup-account");

    // Mark backup in cooldown as well
    pool.mark_rate_limited("claude:backup-account", Duration::from_secs(300))
        .await;

    // Both in cooldown -> leasing fails closed
    assert!(
        pool.lease_account(ModelProvider::AnthropicClaudeCode)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn test_multi_horizon_5hr_and_weekly_accounting() {
    let pool = AccountPoolManager::new();

    // Add custom account with explicit quotas
    let custom = ManagedAccount {
        account_id: "codex:enterprise-01".to_string(),
        provider: ModelProvider::OpenAiCodex,
        auth_type: anvil::self_governance::AuthType::ConfigDirectory,
        auth_profile_or_key: Some("OPENAI_KEY_ENT".to_string()),
        oauth_token: None,
        config_dir: Some("/Users/name/.codex-enterprise".to_string()),
        max_5hr_tokens: Some(1_000_000),
        max_weekly_budget_usd: Some(150.0),
        usage_history: VecDeque::new(),
        cooldown_until: None,
        last_leased_at: Instant::now(),
        is_draining: false,
    };
    pool.add_account(custom).await.unwrap();

    // Record spend
    let quota = pool
        .record_spend("codex:enterprise-01", "gpt-5.6-sol", 250_000, 3.75)
        .await
        .unwrap();

    assert_eq!(quota.used_5hr_tokens, 250_000);
    assert_eq!(quota.remaining_5hr_tokens, Some(750_000));
    assert_eq!(quota.pct_5hr_used, Some(25.0));
    assert_eq!(quota.weekly_spent_usd, 3.75);
    assert!(quota.is_active);

    // Verify status views (5 discovered default CLI accounts + 1 added custom account = 6)
    let views = pool.get_pool_status_views().await;
    assert_eq!(views.len(), 6);
}

#[tokio::test]
async fn test_dynamic_account_addition_and_drain() {
    let pool = AccountPoolManager::new();

    // Add new dynamic account to pool
    let new_acc = ManagedAccount {
        account_id: "claude-dynamic-gamma".to_string(),
        provider: ModelProvider::AnthropicClaudeCode,
        auth_type: anvil::self_governance::AuthType::OAuthToken,
        auth_profile_or_key: Some("CLAUDE_GAMMA_KEY".to_string()),
        oauth_token: Some("sk-ant-oat01-gamma-token".to_string()),
        config_dir: None,
        max_5hr_tokens: Some(1_000_000),
        max_weekly_budget_usd: Some(250.0),
        usage_history: VecDeque::new(),
        cooldown_until: None,
        last_leased_at: Instant::now(),
        is_draining: false,
    };

    pool.add_account(new_acc).await.unwrap();

    // Verify account registered
    let views = pool.get_pool_status_views().await;
    assert!(views.iter().any(|v| v.account_id == "claude-dynamic-gamma"));

    // Drain account
    pool.drain_account("claude-dynamic-gamma").await.unwrap();
    let views_draining = pool.get_pool_status_views().await;
    let gamma_view = views_draining
        .iter()
        .find(|v| v.account_id == "claude-dynamic-gamma")
        .unwrap();
    assert!(gamma_view.is_draining);
    assert_eq!(gamma_view.lifecycle_state, "DRAINING");

    // Resume account
    pool.resume_account("claude-dynamic-gamma").await.unwrap();
    let views_resumed = pool.get_pool_status_views().await;
    let gamma_resumed = views_resumed
        .iter()
        .find(|v| v.account_id == "claude-dynamic-gamma")
        .unwrap();
    assert!(!gamma_resumed.is_draining);
    assert_eq!(gamma_resumed.lifecycle_state, "ACTIVE");
}

#[tokio::test]
async fn test_context_cache_affinity_leasing() {
    let pool = AccountPoolManager::new();

    // Add second account
    let custom = ManagedAccount {
        account_id: "claude:secondary".to_string(),
        provider: ModelProvider::AnthropicClaudeCode,
        auth_type: anvil::self_governance::AuthType::CliPassthrough,
        auth_profile_or_key: Some("CLAUDE_SEC".to_string()),
        oauth_token: None,
        config_dir: None,
        max_5hr_tokens: Some(500_000),
        max_weekly_budget_usd: Some(100.0),
        usage_history: VecDeque::new(),
        cooldown_until: None,
        last_leased_at: Instant::now(),
        is_draining: false,
    };
    pool.add_account(custom).await.unwrap();

    // 1. Lease account with context affinity key
    let affinity_key = "repo:oyatie/oyatie#pr-2158";
    let acc1 = pool
        .lease_account_with_affinity(ModelProvider::AnthropicClaudeCode, Some(affinity_key))
        .await
        .unwrap();
    let acc1_id = acc1.read().await.account_id.clone();

    // Record spend on acc1 so it has higher load than acc2
    pool.record_spend(&acc1_id, "claude-opus-5", 200_000, 6.0)
        .await
        .unwrap();

    // 2. Next lease with the SAME affinity key should route to the SAME account despite higher load (prompt cache hit!)
    let acc2 = pool
        .lease_account_with_affinity(ModelProvider::AnthropicClaudeCode, Some(affinity_key))
        .await
        .unwrap();
    let acc2_id = acc2.read().await.account_id.clone();
    assert_eq!(acc1_id, acc2_id);
}
