use anvil::ai_driver::{ModelExecutionConfig, ModelProvider, SubscriptionExecutor};
use anvil::model_prompt::{ModelPrompt, ModelPromptPurpose};
use anvil::reviewer::untrusted::{Untrusted, UntrustedLabel};
use std::path::Path;

fn live_probe(text: &str) -> ModelPrompt {
    let mut prompt = ModelPrompt::builder();
    prompt.push_untrusted(Untrusted::new(UntrustedLabel::ReviewComment, text));
    prompt
        .finish_for(ModelPromptPurpose::SubscriptionProbe)
        .expect("non-empty bounded live prompt")
}

fn is_agy_available() -> bool {
    std::process::Command::new("agy")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn test_live_claude_opus5_high() {
    if !is_agy_available() {
        println!("Skipping live test: agy CLI is not installed on this runner");
        return;
    }

    let executor = SubscriptionExecutor::new();
    let config = ModelExecutionConfig {
        provider: ModelProvider::AnthropicClaudeCode,
        specific_model: Some("opus5".to_string()),
        reasoning_effort: "high".to_string(),
        print_timeout_secs: 60,
    };

    assert_eq!(config.resolved_model(), "opus5");
    let prompt = live_probe("Respond strictly with: OPUS5_HIGH_OK");
    let res = executor
        .execute_prompt(&prompt, Path::new("."), &config)
        .await;

    assert!(
        res.is_ok(),
        "Claude Code Opus 5 execution failed: {:?}",
        res
    );
    let output = res.unwrap();
    println!("Claude Opus 5 Live Output: {}", output);
    assert!(!output.trim().is_empty());
}

#[tokio::test]
async fn test_live_gpt5_6sol_high_with_fallover() {
    if !is_agy_available() {
        println!("Skipping live test: agy CLI is not installed on this runner");
        return;
    }

    let executor = SubscriptionExecutor::new();
    let config = ModelExecutionConfig {
        provider: ModelProvider::OpenAiCodex,
        specific_model: Some("gpt-5.6-sol".to_string()),
        reasoning_effort: "high".to_string(),
        print_timeout_secs: 60,
    };

    assert_eq!(config.resolved_model(), "gpt-5.6-sol");
    let prompt = live_probe("Respond strictly with: GPT5_6SOL_HIGH_OK");
    let res = executor
        .execute_prompt(&prompt, Path::new("."), &config)
        .await;

    assert!(res.is_ok(), "Codex GPT-5.6sol execution failed: {:?}", res);
    let output = res.unwrap();
    println!("GPT-5.6sol Live / Fallover Output: {}", output);
    assert!(!output.trim().is_empty());
}

#[tokio::test]
async fn test_live_gemini3_7_flash_high() {
    if !is_agy_available() {
        println!("Skipping live test: agy CLI is not installed on this runner");
        return;
    }

    let executor = SubscriptionExecutor::new();
    let config = ModelExecutionConfig {
        provider: ModelProvider::Antigravity,
        specific_model: Some("gemini-3.7-flash".to_string()),
        reasoning_effort: "high".to_string(),
        print_timeout_secs: 60,
    };

    assert_eq!(config.resolved_model(), "gemini-3.7-flash");
    let prompt = live_probe("Respond strictly with: GEMINI3_7_FLASH_HIGH_OK");
    let res = executor
        .execute_prompt(&prompt, Path::new("."), &config)
        .await;

    assert!(res.is_ok(), "Gemini 3.7 Flash execution failed: {:?}", res);
    let output = res.unwrap();
    println!("Gemini 3.7 Flash Live Output: {}", output);
    assert!(!output.trim().is_empty());
}
