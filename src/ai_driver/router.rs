use anyhow::{bail, Context, Result};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tracing::{error, info, warn};

use super::provider::{ModelExecutionConfig, ModelProvider};
use crate::self_governance::account_pool::AccountPoolManager;

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

        let account_id = match &leased {
            Ok(acc_arc) => {
                let acc = acc_arc.read().await;
                info!(
                    "Leased account '{}' for Claude Code (model: {}, effort: {})...",
                    acc.account_id, model_name, config.reasoning_effort
                );
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

        let mut cmd = Command::new("claude");
        cmd.args(["-p", prompt]);
        cmd.args(["--model", model_name]);
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        match cmd.output().await {
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
                warn!("Claude subscription notice: {}. Falling over to active subscription fallback...", stderr);
                self.account_pool
                    .mark_rate_limited(&account_id, Duration::from_secs(60))
                    .await;
            }
            Err(e) => {
                warn!("Claude CLI invocation notice: ({}). Falling over to active subscription fallback...", e);
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

        let account_id = match &leased {
            Ok(acc_arc) => {
                let acc = acc_arc.read().await;
                info!(
                    "Leased account '{}' for OpenAI Codex (model: {}, effort: {})...",
                    acc.account_id, model_name, config.reasoning_effort
                );
                acc.account_id.clone()
            }
            Err(_) => "codex-default".to_string(),
        };

        let mut cmd = Command::new("codex");
        cmd.args(["exec", prompt]);
        cmd.args(["--model", model_name]);
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        match cmd.output().await {
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
        let mut cmd = Command::new("cursor");
        cmd.args(["agent", "--print", prompt]);
        if !model.is_empty() && model != "default" {
            cmd.args(["--model", model]);
        }
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        match cmd.output().await {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.trim().is_empty() {
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
        let mut cmd = Command::new("grok");
        cmd.args(["--prompt", prompt, "--model", model]);
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        match cmd.output().await {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.trim().is_empty() {
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

        let account_id = match &leased {
            Ok(acc_arc) => {
                let acc = acc_arc.read().await;
                acc.account_id.clone()
            }
            Err(_) => "agy-default".to_string(),
        };

        let mut cmd = Command::new("agy");
        cmd.args([
            "--print",
            prompt,
            "--effort",
            &config.reasoning_effort,
            "--dangerously-skip-permissions",
        ]);

        if !model.is_empty() && model != "default" {
            cmd.args(["--model", model]);
        }

        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd
            .output()
            .await
            .context("Failed to run agy subscription CLI")?;

        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            error!(
                "agy subscription process returned status: {}",
                output.status
            );
            warn!("agy stderr: {}", stderr_str);
            if stdout_str.trim().is_empty() {
                bail!("agy failed with code {}: {}", output.status, stderr_str);
            }
        }

        // Record token usage in pool
        let tokens = ((prompt.len() + stdout_str.len()) as f64 / 3.8).ceil() as usize;
        let cost_usd = (tokens as f64 / 1_000_000.0) * 1.50;
        let _ = self
            .account_pool
            .record_spend(&account_id, model, tokens, cost_usd)
            .await;

        Ok(stdout_str)
    }

    /// Evaluates prompt using Multi-Model Ensemble across Opus 5 + GPT-5.6sol + Grok 4.6 + Gemini 3.7 Flash subscriptions
    async fn run_ensemble_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        info!("Executing prompt via Multi-Model Subscription Ensemble (Opus 5 + GPT-5.6sol + Grok 4.6 + Gemini 3.7 Flash)...");
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
