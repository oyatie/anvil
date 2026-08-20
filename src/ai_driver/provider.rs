use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ModelProvider {
    #[default]
    AnthropicClaudeCode,
    OpenAiCodex,
    CursorAgent,
    XAiGrok,
    Antigravity,
    SubscriptionEnsemble,
}

impl ModelProvider {
    pub fn from_str_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "claude" | "claude-code" | "anthropic" | "opus5" | "claude-opus-5" => {
                ModelProvider::AnthropicClaudeCode
            }
            "codex" | "gpt" | "openai" | "gpt5" | "gpt5.6sol" | "gpt-5.6-sol" | "chatgpt" => {
                ModelProvider::OpenAiCodex
            }
            "cursor" | "cursor-agent" | "agent" => ModelProvider::CursorAgent,
            "grok" | "grok4" | "grok4.6" | "grok4.6high" | "xai" => ModelProvider::XAiGrok,
            "ensemble" | "multi" | "hybrid" => ModelProvider::SubscriptionEnsemble,
            "gemini" | "gemini3.7" | "gemini-3.7-flash" => ModelProvider::Antigravity,
            _ => ModelProvider::Antigravity,
        }
    }

    pub fn default_frontier_model(&self) -> &'static str {
        match self {
            ModelProvider::AnthropicClaudeCode => "opus5",
            ModelProvider::OpenAiCodex => "gpt-5.6-sol",
            ModelProvider::XAiGrok => "grok-4.6",
            ModelProvider::Antigravity => "gemini-3.7-flash",
            ModelProvider::CursorAgent => "gpt-5.6-sol",
            ModelProvider::SubscriptionEnsemble => "opus5",
        }
    }

    pub fn default_reasoning_effort(&self) -> &'static str {
        "high"
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ModelProvider::AnthropicClaudeCode => "Anthropic Claude Code Subscription (Opus 5 - High Effort)",
            ModelProvider::OpenAiCodex => "OpenAI Codex Subscription (GPT-5.6sol - High Effort)",
            ModelProvider::CursorAgent => "Cursor Agent Subscription (Multi-Model Native - High Effort)",
            ModelProvider::XAiGrok => "xAI Grok Subscription (Grok 4.6 - High Effort)",
            ModelProvider::Antigravity => "Google Antigravity Subscription (Gemini 3.7 Flash - High Effort)",
            ModelProvider::SubscriptionEnsemble => "Multi-Model Subscription Ensemble (Opus 5 + GPT-5.6sol + Grok 4.6 + Gemini 3.7 Flash)",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelExecutionConfig {
    pub provider: ModelProvider,
    pub specific_model: Option<String>,
    pub reasoning_effort: String,
    pub print_timeout_secs: u64,
}

impl Default for ModelExecutionConfig {
    fn default() -> Self {
        let provider = ModelProvider::AnthropicClaudeCode;
        let model = provider.default_frontier_model().to_string();
        let effort = provider.default_reasoning_effort().to_string();
        Self {
            provider,
            specific_model: Some(model),
            reasoning_effort: effort,
            print_timeout_secs: 300,
        }
    }
}

impl ModelExecutionConfig {
    pub fn resolved_model(&self) -> &str {
        self.specific_model
            .as_deref()
            .unwrap_or_else(|| self.provider.default_frontier_model())
    }
}
