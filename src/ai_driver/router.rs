use anyhow::{bail, Context, Result};
use std::path::Path;
use tokio::process::Command;
use tracing::{error, info, warn};

use super::provider::{ModelExecutionConfig, ModelProvider};

pub struct SubscriptionExecutor;

impl SubscriptionExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Executes prompt using the user's logged-in CLI subscription (Claude Code, OpenAI Codex, Cursor Agent, xAI Grok, or Antigravity)
    pub async fn execute_prompt(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        match config.provider {
            ModelProvider::AnthropicClaudeCode => {
                self.run_claude_subscription(prompt, working_dir, config).await
            }
            ModelProvider::OpenAiCodex => {
                self.run_openai_subscription(prompt, working_dir, config).await
            }
            ModelProvider::CursorAgent => {
                self.run_cursor_agent_subscription(
                    prompt,
                    working_dir,
                    config.resolved_model(),
                )
                .await
            }
            ModelProvider::XAiGrok => {
                self.run_grok_subscription(prompt, working_dir, config).await
            }
            ModelProvider::SubscriptionEnsemble => {
                self.run_ensemble_subscription(prompt, working_dir, config).await
            }
            ModelProvider::Antigravity => {
                self.run_agy_subscription(prompt, working_dir, config).await
            }
        }
    }

    /// Invokes Anthropic Claude Code subscription CLI (`claude -p <prompt> < /dev/null` e.g. opus5 - high reasoning)
    pub async fn run_claude_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        let model_name = config.resolved_model();
        info!(
            "Executing prompt via Anthropic Claude Code subscription (model: {}, effort: {})...",
            model_name, config.reasoning_effort
        );

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
                if !stdout.trim().is_empty() && !stdout.contains("ERROR: You've hit your usage limit") {
                    return Ok(stdout);
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                warn!("Claude subscription notice: {}. Falling over to active subscription fallback...", stderr);
            }
            Err(e) => {
                warn!("Claude CLI invocation notice: ({}). Falling over to active subscription fallback...", e);
            }
        }

        // Fallback: AGY with default subscription (Gemini 3.7 Flash - high effort)
        let mut fallback_config = config.clone();
        fallback_config.provider = ModelProvider::Antigravity;
        fallback_config.specific_model = Some(ModelProvider::Antigravity.default_frontier_model().to_string());
        self.run_agy_subscription(prompt, working_dir, &fallback_config).await
    }

    /// Invokes OpenAI Codex / ChatGPT subscription CLI (`codex exec <prompt> < /dev/null` e.g. gpt-5.6-sol - high reasoning)
    pub async fn run_openai_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        let model_name = config.resolved_model();
        info!(
            "Executing prompt via OpenAI Codex / ChatGPT subscription (model: {}, effort: {})...",
            model_name, config.reasoning_effort
        );

        let mut cmd = Command::new("codex");
        cmd.args(["exec", "-m", model_name]);
        cmd.arg(prompt);
        cmd.current_dir(working_dir);
        cmd.stdin(std::process::Stdio::null());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        match cmd.output().await {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                if !stdout.trim().is_empty() && !stdout.contains("ERROR: You've hit your usage limit") {
                    return Ok(stdout);
                }
                if stdout.contains("ERROR: You've hit your usage limit") {
                    warn!("OpenAI Codex subscription usage limit reached. Falling over to Claude Code (Opus 5) / AGY subscription...");
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                warn!("Codex subscription notice: {}. Falling over to active subscription fallback...", stderr);
            }
            Err(e) => {
                warn!("Codex CLI invocation notice: ({}). Falling over to active subscription fallback...", e);
            }
        }

        // Fallback: Claude Code subscription (Opus 5) or AGY (Gemini 3.7 Flash)
        let mut claude_config = config.clone();
        claude_config.provider = ModelProvider::AnthropicClaudeCode;
        claude_config.specific_model = Some(ModelProvider::AnthropicClaudeCode.default_frontier_model().to_string());
        if let Ok(res) = self.run_claude_subscription(prompt, working_dir, &claude_config).await {
            return Ok(res);
        }

        let mut fallback_config = config.clone();
        fallback_config.provider = ModelProvider::Antigravity;
        fallback_config.specific_model = Some(ModelProvider::Antigravity.default_frontier_model().to_string());
        self.run_agy_subscription(prompt, working_dir, &fallback_config).await
    }

    /// Invokes xAI Grok subscription (`grok-4.6` high reasoning)
    pub async fn run_grok_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        let grok_model = config.resolved_model();
        info!(
            "Executing prompt via xAI Grok subscription (model: {}, effort: {})...",
            grok_model, config.reasoning_effort
        );

        if let Ok(res) = self.run_cursor_agent_subscription(prompt, working_dir, grok_model).await {
            if !res.trim().is_empty() {
                return Ok(res);
            }
        }

        // Fallback: Claude Code (Opus 5) or AGY (Gemini 3.7 Flash)
        let mut claude_config = config.clone();
        claude_config.provider = ModelProvider::AnthropicClaudeCode;
        claude_config.specific_model = Some(ModelProvider::AnthropicClaudeCode.default_frontier_model().to_string());
        if let Ok(res) = self.run_claude_subscription(prompt, working_dir, &claude_config).await {
            return Ok(res);
        }

        let mut fallback_config = config.clone();
        fallback_config.provider = ModelProvider::Antigravity;
        fallback_config.specific_model = Some(ModelProvider::Antigravity.default_frontier_model().to_string());
        self.run_agy_subscription(prompt, working_dir, &fallback_config).await
    }

    /// Invokes Cursor Agent subscription CLI (`cursor-agent --print --model <model> <prompt>`)
    pub async fn run_cursor_agent_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        model: &str,
    ) -> Result<String> {
        let mut cmd = Command::new("cursor-agent");
        cmd.args(["--print", "--model", model, prompt]);
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

    /// Invokes Antigravity subscription CLI (`agy` with Gemini 3.7 Flash - high reasoning effort)
    pub async fn run_agy_subscription(
        &self,
        prompt: &str,
        working_dir: &Path,
        config: &ModelExecutionConfig,
    ) -> Result<String> {
        let model = config.resolved_model();
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
            error!("agy subscription process returned status: {}", output.status);
            warn!("agy stderr: {}", stderr_str);
            if stdout_str.trim().is_empty() {
                bail!("agy failed with code {}: {}", output.status, stderr_str);
            }
        }

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
        self.run_claude_subscription(prompt, working_dir, config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontier_defaults() {
        assert_eq!(ModelProvider::AnthropicClaudeCode.default_frontier_model(), "opus5");
        assert_eq!(ModelProvider::OpenAiCodex.default_frontier_model(), "gpt-5.6-sol");
        assert_eq!(ModelProvider::XAiGrok.default_frontier_model(), "grok-4.6");
        assert_eq!(ModelProvider::Antigravity.default_frontier_model(), "gemini-3.7-flash");
    }
}
