use anyhow::{Result, bail};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tracing::{error, info, warn};

use super::provider::{ModelExecutionConfig, ModelProvider};
use crate::self_governance::account_pool::AccountPoolManager;

/// Delivers a prompt to a provider CLI over STDIN instead of argv.
///
/// argv is not a safe channel for a review prompt: Linux caps a single
/// argument at MAX_ARG_STRLEN (~128KB) and Darwin caps the whole argv block at
/// ARG_MAX (1MB), so a large diff makes the spawn itself fail with E2BIG. That
/// failure arrives as an empty stdout, which `reviewer::parse_review_response`
/// then has to interpret -- a spawn error that is indistinguishable from model
/// output is the shape of defect this crate exists to eliminate.
///
/// The bound comes from `crate::exec`, which owns the timeout and
/// `kill_on_drop` (invariant I5). Nothing in this module spawns a child or
/// times one out on its own.
pub async fn run_with_prompt_on_stdin(
    cmd: Command,
    prompt: &str,
    limit: Duration,
    what: &str,
) -> Result<std::process::Output> {
    crate::exec::run_bounded_with_stdin(cmd, prompt, limit, what).await
}

#[derive(Debug, Clone)]
pub struct SubscriptionExecutor {
    account_pool: Arc<AccountPoolManager>,
}

impl Default for SubscriptionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionExecutor {
    pub fn new() -> Self {
        Self {
            account_pool: Arc::new(AccountPoolManager::new()),
        }
    }

    pub fn with_pool(account_pool: Arc<AccountPoolManager>) -> Self {
        Self { account_pool }
    }

    /// Executes prompt using the user's logged-in CLI subscription with multi-account pooling and failover
    pub async fn execute_prompt(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        match config.provider {
            ModelProvider::AnthropicClaudeCode => {
                self.run_claude_subscription(prompt, working_dir, config)
                    .await
            }
            ModelProvider::OpenAiCodex => {
                self.run_openai_subscription(prompt, working_dir, config)
                    .await
            }
            ModelProvider::CursorAgent => {
                self.run_cursor_agent_subscription(prompt, working_dir, config.resolved_model())
                    .await
            }
            ModelProvider::XAiGrok => {
                self.run_grok_subscription(prompt, working_dir, config)
                    .await
            }
            ModelProvider::SubscriptionEnsemble => {
                self.run_ensemble_subscription(prompt, working_dir, config)
                    .await
            }
            ModelProvider::Antigravity => {
                self.run_agy_subscription(prompt, working_dir, config).await
            }
        }
    }

    /// Invokes Anthropic Claude Code subscription CLI with multi-account pool leasing and failover
    pub async fn run_claude_subscription(
        &self,
        prompt: &str,
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
                    posture = posture.with_credential("CLAUDE_CONFIG_DIR", dir);
                }
                if let Some(tok) = &acc.oauth_token {
                    posture = posture
                        .with_credential("CLAUDE_CODE_OAUTH_TOKEN", tok)
                        .with_credential("ANTHROPIC_AUTH_TOKEN", tok);
                }
                // Let-chain, stable in edition 2024: the HOST_ prefix marks a
                // host-managed profile name rather than a key, and must never be
                // exported as one.
                if let Some(key) = &acc.auth_profile_or_key
                    && !key.starts_with("HOST_")
                {
                    posture = posture.with_credential("ANTHROPIC_API_KEY", key);
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

        let mut cmd = crate::exec::agent("claude", &posture);
        // `-p` with no positional argument: the prompt arrives on STDIN.
        cmd.arg("-p");
        cmd.args(["--model", model_name]);

        match run_with_prompt_on_stdin(
            cmd,
            prompt,
            std::time::Duration::from_secs(config.print_timeout_secs),
            "provider CLI",
        )
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

    /// Invokes OpenAI Codex / ChatGPT subscription CLI with multi-account pool leasing and failover
    pub async fn run_openai_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        let model_name = config.resolved_model();
        let leased = self
            .account_pool
            .lease_account(ModelProvider::OpenAiCodex)
            .await;

        let mut posture = crate::exec::Posture::in_workspace(working_dir);
        let account_id = match &leased {
            Ok(acc_arc) => {
                let acc = acc_arc.read().await;
                info!(
                    "Leased account '{}' for OpenAI Codex (model: {}, effort: {})...",
                    acc.account_id, model_name, config.reasoning_effort
                );
                if let Some(dir) = &acc.config_dir {
                    posture = posture.with_credential("CODEX_HOME", dir);
                }
                if let Some(tok) = &acc.oauth_token {
                    posture = posture
                        .with_credential("OPENAI_AUTH_TOKEN", tok)
                        .with_credential("CODEX_AUTH_TOKEN", tok);
                }
                // Let-chain, stable in edition 2024: the HOST_ prefix marks a
                // host-managed profile name rather than a key, and must never be
                // exported as one.
                if let Some(key) = &acc.auth_profile_or_key
                    && !key.starts_with("HOST_")
                {
                    posture = posture.with_credential("OPENAI_API_KEY", key);
                }
                acc.account_id.clone()
            }
            Err(_) => "codex-default".to_string(),
        };

        let mut cmd = crate::exec::agent("codex", &posture);
        // `-` is codex's explicit "read the prompt from STDIN" argument.
        cmd.args(["exec", "-"]);
        cmd.args(["--model", model_name]);

        match run_with_prompt_on_stdin(
            cmd,
            prompt,
            std::time::Duration::from_secs(config.print_timeout_secs),
            "provider CLI",
        )
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.trim().is_empty() {
                    let tokens = ((prompt.len() + stdout.len()) as f64 / 3.8).ceil() as usize;
                    let cost_usd = (tokens as f64 / 1_000_000.0) * 15.0;
                    let _ = self
                        .account_pool
                        .record_spend(&account_id, model_name, tokens, cost_usd)
                        .await;
                    return Ok(stdout);
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                warn!("OpenAI Codex subscription notice: {}", stderr);
                self.account_pool
                    .mark_rate_limited(&account_id, Duration::from_secs(60))
                    .await;
            }
            Err(e) => {
                warn!("OpenAI Codex invocation notice: {}", e);
            }
        }

        // Fallback: AGY with default subscription
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

    /// Invokes Cursor Agent subscription CLI
    pub async fn run_cursor_agent_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        model: &str,
    ) -> Result<String> {
        let leased = self
            .account_pool
            .lease_account(ModelProvider::CursorAgent)
            .await;

        let mut posture = crate::exec::Posture::in_workspace(working_dir);
        let account_id = match &leased {
            Ok(acc_arc) => {
                let acc = acc_arc.read().await;
                if let Some(dir) = &acc.config_dir {
                    posture = posture.with_credential("CURSOR_CONFIG_DIR", dir);
                }
                if let Some(tok) = &acc.oauth_token {
                    posture = posture.with_credential("CURSOR_AUTH_TOKEN", tok);
                }
                acc.account_id.clone()
            }
            Err(_) => "cursor-default".to_string(),
        };

        let mut cmd = crate::exec::agent("cursor", &posture);
        // No positional prompt: it is written to STDIN below.
        cmd.args(["agent", "--print"]);
        if !model.is_empty() && model != "default" {
            cmd.args(["--model", model]);
        }

        match run_with_prompt_on_stdin(
            cmd,
            prompt,
            crate::exec::ExecClass::Model.timeout(),
            "provider CLI",
        )
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.trim().is_empty() {
                    let tokens = ((prompt.len() + stdout.len()) as f64 / 3.8).ceil() as usize;
                    let cost_usd = (tokens as f64 / 1_000_000.0) * 20.0;
                    let _ = self
                        .account_pool
                        .record_spend(&account_id, model, tokens, cost_usd)
                        .await;
                    return Ok(stdout);
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                warn!("Cursor Agent model {} notice: {}", model, stderr);
            }
            Err(e) => {
                warn!("Cursor Agent execution notice: {}", e);
            }
        }

        bail!("Cursor Agent execution returned empty or non-zero status")
    }

    /// Invokes xAI Grok subscription CLI
    pub async fn run_grok_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        let model = config.resolved_model();

        let leased = self
            .account_pool
            .lease_account(ModelProvider::XAiGrok)
            .await;

        let mut posture = crate::exec::Posture::in_workspace(working_dir);
        let account_id = match &leased {
            Ok(acc_arc) => {
                let acc = acc_arc.read().await;
                if let Some(dir) = &acc.config_dir {
                    posture = posture.with_credential("GROK_CONFIG_DIR", dir);
                }
                if let Some(tok) = &acc.oauth_token {
                    posture = posture
                        .with_credential("GROK_AUTH_TOKEN", tok)
                        .with_credential("XAI_API_KEY", tok);
                }
                // Let-chain, stable in edition 2024: the HOST_ prefix marks a
                // host-managed profile name rather than a key, and must never be
                // exported as one.
                if let Some(key) = &acc.auth_profile_or_key
                    && !key.starts_with("HOST_")
                {
                    posture = posture.with_credential("XAI_API_KEY", key);
                }
                acc.account_id.clone()
            }
            Err(_) => "grok-default".to_string(),
        };

        let mut cmd = crate::exec::agent("grok", &posture);
        // grok takes its single-turn prompt as a positional argument or from a
        // file. `/dev/stdin` is the file that IS the pipe, so the prompt still
        // travels on STDIN and argv stays a fixed dozen bytes. Verified against
        // the installed CLI; `--prompt <text>` is not a flag this CLI has.
        cmd.args(["--prompt-file", "/dev/stdin", "--model", model]);

        match run_with_prompt_on_stdin(
            cmd,
            prompt,
            std::time::Duration::from_secs(config.print_timeout_secs),
            "provider CLI",
        )
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))
        {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.trim().is_empty() {
                    let tokens = ((prompt.len() + stdout.len()) as f64 / 3.8).ceil() as usize;
                    let cost_usd = (tokens as f64 / 1_000_000.0) * 10.0;
                    let _ = self
                        .account_pool
                        .record_spend(&account_id, model, tokens, cost_usd)
                        .await;
                    return Ok(stdout);
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                warn!("xAI Grok subscription notice: {}", stderr);
            }
            Err(e) => {
                warn!("xAI Grok execution notice: {}", e);
            }
        }

        // Fallback: AGY
        self.run_agy_subscription(prompt, working_dir, config).await
    }

    /// Invokes Antigravity subscription CLI (`agy` with Gemini 3.7 Flash - high reasoning effort)
    pub async fn run_agy_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        let model = config.resolved_model();

        let leased = self
            .account_pool
            .lease_account(ModelProvider::Antigravity)
            .await;

        let turn_limit = std::time::Duration::from_secs(config.print_timeout_secs);

        let mut posture = crate::exec::Posture::in_workspace(working_dir);
        let account_id = match &leased {
            Ok(acc_arc) => {
                let acc = acc_arc.read().await;
                if let Some(dir) = &acc.config_dir {
                    posture = posture
                        .with_credential("ANTIGRAVITY_CONFIG_DIR", dir)
                        .with_credential("GEMINI_CLI_CONFIG_DIR", dir);
                }
                if let Some(tok) = &acc.oauth_token {
                    posture = posture
                        .with_credential("ANTIGRAVITY_AUTH_TOKEN", tok)
                        .with_credential("GEMINI_API_KEY", tok);
                }
                // Let-chain, stable in edition 2024: the HOST_ prefix marks a
                // host-managed profile name rather than a key, and must never be
                // exported as one.
                if let Some(key) = &acc.auth_profile_or_key
                    && !key.starts_with("HOST_")
                {
                    posture = posture.with_credential("GEMINI_API_KEY", key);
                }
                acc.account_id.clone()
            }
            Err(_) => "agy-default".to_string(),
        };

        let mut cmd = crate::exec::agent("agy", &posture);
        crate::exec::turn::agy_turn(&mut cmd, &config.reasoning_effort, turn_limit);

        if !model.is_empty() && model != "default" {
            cmd.args(["--model", model]);
        }

        // print_timeout_secs was set in 23 places and read nowhere; it now
        // actually bounds the call (invariant I5).
        let turn = crate::exec::turn::run(cmd, prompt, turn_limit, "agy subscription CLI").await?;

        if !turn.status.success() {
            error!("agy subscription process returned status: {}", turn.status);
            warn!("agy stderr: {}", turn.stderr);
        }

        // A non-zero exit fails the call outright rather than falling through
        // on whatever stdout arrived: a stream truncated by
        // `Error: timeout waiting for response` would otherwise be handed to
        // the parser below and become a review verdict -- a judgement assembled
        // from however much of the model's answer happened to arrive.
        let response = turn.into_result()?;

        // Record token usage in pool
        let tokens = ((prompt.len() + response.len()) as f64 / 3.8).ceil() as usize;
        let cost_usd = (tokens as f64 / 1_000_000.0) * 1.50;
        let _ = self
            .account_pool
            .record_spend(&account_id, model, tokens, cost_usd)
            .await;

        Ok(response)
    }

    /// Evaluates prompt using Multi-Model Ensemble across Opus 5 + GPT-5.6sol + Grok 4.6 + Gemini 3.7 Flash subscriptions
    async fn run_ensemble_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        info!(
            "Executing prompt via Multi-Model Subscription Ensemble (Opus 5 + GPT-5.6sol + Grok 4.6 + Gemini 3.7 Flash)..."
        );
        self.run_claude_subscription(prompt, working_dir, config)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontier_defaults() {
        assert_eq!(
            ModelProvider::AnthropicClaudeCode.default_frontier_model(),
            "opus5"
        );
        assert_eq!(
            ModelProvider::OpenAiCodex.default_frontier_model(),
            "gpt-5.6-sol"
        );
    }
}
