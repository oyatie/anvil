use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

use crate::ai_driver::provider::ModelProvider;
use super::types::{AuthType, ManagedAccount};

/// Creates the 5 default CLI passthrough accounts from the host environment.
/// Each provider gets one account bootstrapped from the locally-authenticated CLI session.
pub fn bootstrap_default_accounts() -> Vec<(ModelProvider, Vec<Arc<RwLock<ManagedAccount>>>)> {
    let providers = [
        (
            ModelProvider::AnthropicClaudeCode,
            "claude:cli-default",
            "HOST_CLAUDE_CLI_AUTH",
        ),
        (
            ModelProvider::OpenAiCodex,
            "codex:cli-default",
            "HOST_CODEX_CLI_AUTH",
        ),
        (
            ModelProvider::Antigravity,
            "agy:cli-default",
            "HOST_AGY_CLI_AUTH",
        ),
        (
            ModelProvider::CursorAgent,
            "cursor:cli-default",
            "HOST_CURSOR_CLI_AUTH",
        ),
        (
            ModelProvider::XAiGrok,
            "grok:cli-default",
            "HOST_GROK_CLI_AUTH",
        ),
    ];

    providers
        .into_iter()
        .map(|(provider, account_id, auth_key)| {
            let account = Arc::new(RwLock::new(ManagedAccount {
                account_id: account_id.to_string(),
                provider: provider.clone(),
                auth_type: AuthType::CliPassthrough,
                auth_profile_or_key: Some(auth_key.to_string()),
                oauth_token: None,
                config_dir: None,
                max_5hr_tokens: None,
                max_weekly_budget_usd: None,
                usage_history: VecDeque::new(),
                cooldown_until: None,
                last_leased_at: Instant::now(),
                is_draining: false,
            }));
            (provider, vec![account])
        })
        .collect()
}
