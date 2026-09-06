use super::{SubscriptionExecutor, run_with_prompt_on_stdin};
use crate::ai_driver::provider::{ModelExecutionConfig, ModelProvider};
use crate::exec::ProviderCredential;
use crate::model_prompt::ModelPrompt;
use anyhow::Result;
use std::path::Path;
use std::time::Duration;
use tracing::{info, warn};

impl SubscriptionExecutor {
    /// Invokes Anthropic Claude Code subscription CLI with multi-account pool leasing and failover
    pub async fn run_claude_subscription(
        &self,
        prompt: &ModelPrompt,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        let model_name = config.resolved_model();

        // Lease account from pool
        let leased = self
            .account_pool
            .lease_account(ModelProvider::AnthropicClaudeCode)
            .await;
        // The lease is read before the spawn, because a leased credential is
        // part of the posture rather than something added to a command that
        // already exists.
        let mut posture = crate::exec::Posture::in_workspace(working_dir);
        let account_id = match &leased {
            Ok(acc_arc) => {
                let acc = acc_arc.read().await;
                info!(
                    "Leased account '{}' for Claude Code (model: {}, effort: {})...",
                    acc.account_id, model_name, config.reasoning_effort
                );
                if let Some(dir) = &acc.config_dir {
                    posture = posture.with_credential(ProviderCredential::ClaudeConfigDir, dir);
                }
                if let Some(tok) = &acc.oauth_token {
                    posture = posture
                        .with_credential(ProviderCredential::ClaudeCodeOauthToken, tok)
                        .with_credential(ProviderCredential::AnthropicAuthToken, tok);
                }
                // Let-chain, stable in edition 2024: the HOST_ prefix marks a
                // host-managed profile name rather than a key, and must never be
                // exported as one.
                if let Some(key) = &acc.auth_profile_or_key
                    && !key.starts_with("HOST_")
                {
                    posture = posture.with_credential(ProviderCredential::AnthropicApiKey, key);
                }
                acc.account_id.clone()
            }
            Err(e) => {
                warn!(
                    "Claude account pool notice ({}). Falling over to AGY fallback...",
                    e
                );
                "claude-default".to_string()
            }
        };

        match async {
            let cmd = crate::exec::claude_agent(&posture, model_name)?;
            run_with_prompt_on_stdin(
                cmd,
                prompt,
                std::time::Duration::from_secs(config.print_timeout_secs),
                "provider CLI",
            )
            .await
        }
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.trim().is_empty()
                    && !stdout.contains("ERROR: You've hit your usage limit")
                {
                    // Record token usage in pool
                    let tokens = ((prompt.len() + stdout.len()) as f64 / 3.8).ceil() as usize;
                    let cost_usd = (tokens as f64 / 1_000_000.0) * 30.0;
                    let _ = self
                        .account_pool
                        .record_spend(&account_id, model_name, tokens, cost_usd)
                        .await;
                    return Ok(stdout);
                } else if stdout.contains("ERROR: You've hit your usage limit") {
                    warn!(
                        "Account '{}' hit Claude usage limit. Marking cooldown...",
                        account_id
                    );
                    self.account_pool
                        .mark_rate_limited(&account_id, Duration::from_secs(300))
                        .await;
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                warn!(
                    "Claude subscription notice: {}. Falling over to active subscription fallback...",
                    stderr
                );
                self.account_pool
                    .mark_rate_limited(&account_id, Duration::from_secs(60))
                    .await;
            }
            Err(e) => {
                warn!(
                    "Claude CLI invocation notice: ({}). Falling over to active subscription fallback...",
                    e
                );
            }
        }

        // Fallback: AGY with default subscription (Gemini 3.7 Flash - high effort)
        let mut fallback_config = config.clone();
        fallback_config.provider = ModelProvider::Antigravity;
        fallback_config.specific_model = Some(
            ModelProvider::Antigravity
                .default_frontier_model()
                .to_string(),
        );
        self.run_agy_subscription(prompt, working_dir, &fallback_config)
            .await
    }
}
