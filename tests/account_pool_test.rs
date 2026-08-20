use anvil::ai_driver::provider::ModelProvider;
use anvil::self_governance::account_pool::AccountPoolManager;
use std::time::Duration;

#[tokio::test]
async fn test_multi_account_pool_least_loaded_leasing() {
    let pool = AccountPoolManager::new();

    // 1. Lease first Claude account
    let acc1 = pool
        .lease_account(ModelProvider::AnthropicClaudeCode)
        .await
        .unwrap();
    let acc1_id = acc1.read().await.account_id.clone();
    assert_eq!(acc1_id, "claude-pool-alpha");

    // 2. Record 100k tokens on alpha ($3.00)
    let quota1 = pool
        .record_spend(&acc1_id, "claude-opus-5", 100_000, 3.0)
        .await
        .unwrap();
    assert_eq!(quota1.used_5hr_tokens, 100_000);
    assert_eq!(quota1.remaining_5hr_tokens, 400_000);

    // 3. Lease next Claude account -> should pick claude-pool-beta (0 tokens used)
    let acc2 = pool
        .lease_account(ModelProvider::AnthropicClaudeCode)
        .await
        .unwrap();
    let acc2_id = acc2.read().await.account_id.clone();
    assert_eq!(acc2_id, "claude-pool-beta");
}

#[tokio::test]
async fn test_multi_account_rate_limit_failover() {
    let pool = AccountPoolManager::new();

    // Mark alpha in cooldown
    pool.mark_rate_limited("claude-pool-alpha", Duration::from_secs(300))
        .await;

    // Lease account -> alpha skipped, beta leased
    let acc = pool
        .lease_account(ModelProvider::AnthropicClaudeCode)
        .await
        .unwrap();
    assert_eq!(acc.read().await.account_id, "claude-pool-beta");

    // Mark beta in cooldown as well
    pool.mark_rate_limited("claude-pool-beta", Duration::from_secs(300))
        .await;

    // Both in cooldown -> leasing fails closed
    assert!(pool
        .lease_account(ModelProvider::AnthropicClaudeCode)
        .await
        .is_err());
}

#[tokio::test]
async fn test_multi_horizon_5hr_and_weekly_accounting() {
    let pool = AccountPoolManager::new();

    // Record spend on primary codex account
    let quota = pool
        .record_spend("codex-pool-primary", "gpt-5.6-sol", 250_000, 3.75)
        .await
        .unwrap();

    assert_eq!(quota.used_5hr_tokens, 250_000);
    assert_eq!(quota.remaining_5hr_tokens, 750_000);
    assert_eq!(quota.pct_5hr_used, 25.0);
    assert_eq!(quota.weekly_spent_usd, 3.75);
    assert!(quota.is_active);

    // Verify status views
    let views = pool.get_pool_status_views().await;
    assert_eq!(views.len(), 6); // 2 Claude, 2 Codex, 2 AGY
}
