use super::*;

#[test]
fn unavailable_primary_providers_select_the_independently_checked_agy_fallback() {
    const CHILD: &str = "ANVIL_ROUTER_PROVIDER_FREE_TEST";
    if std::env::var_os(CHILD).is_none() {
        let empty_path = tempfile::tempdir().expect("empty provider search directory");
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .env_clear()
            .env("PATH", empty_path.path())
            .env(CHILD, "1")
            .args([
                "ai_driver::router::tests::unavailable_primary_providers_select_the_independently_checked_agy_fallback",
                "--exact",
            ])
            .output()
            .expect("run only the isolated provider-free test");
        assert!(
            output.status.success(),
            "provider-free router regression: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let working_dir = std::env::temp_dir();
    let posture = crate::exec::Posture::in_workspace(&working_dir);
    // Before entering either real route, prove every finite constructor
    // is unavailable. No provider can run, including the selected fallback.
    assert!(crate::exec::claude_agent(&posture, "opus5").is_err());
    assert!(crate::exec::codex_agent(&posture, "gpt-5.6-sol").is_err());
    assert!(crate::exec::agy_agent(&posture, "low", Duration::from_secs(1), None).is_err());
    let mut builder = ModelPrompt::builder();
    builder.push_harness(crate::model_prompt::HarnessText::ReviewerResponseFormat);
    let prompt = builder.finish().expect("finite inert prompt");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("router test runtime");
    runtime.block_on(async {
        let executor = SubscriptionExecutor::new();
        for provider in [
            ModelProvider::AnthropicClaudeCode,
            ModelProvider::OpenAiCodex,
        ] {
            let config = ModelExecutionConfig {
                provider,
                specific_model: None,
                reasoning_effort: "low".to_owned(),
                print_timeout_secs: 1,
            };
            let error = executor
                .execute_prompt(&prompt, &working_dir, &config)
                .await
                .expect_err("the fallback must be independently validated too");
            assert!(
                error
                    .to_string()
                    .contains("provider executable \"agy\" is unavailable"),
                "the missing primary must reach AGY selection: {error}"
            );
        }
    });
}

#[test]
fn test_frontier_defaults() {
    for (provider, expected) in [
        (ModelProvider::AnthropicClaudeCode, "claude-opus-5"),
        (ModelProvider::OpenAiCodex, "gpt-5.6-sol"),
        (ModelProvider::CursorAgent, "gpt-5.6-sol"),
        (ModelProvider::XAiGrok, "grok-4.6"),
        (ModelProvider::Antigravity, "gemini-3.8-flash"),
        (ModelProvider::SubscriptionEnsemble, "claude-opus-5"),
    ] {
        assert_eq!(provider.default_frontier_model(), expected);
        let mut config = ModelExecutionConfig {
            provider,
            specific_model: None,
            reasoning_effort: "high".to_owned(),
            print_timeout_secs: 1,
        };
        assert_eq!(config.resolved_model(), expected);
        config.specific_model = Some("explicit-model-override".to_owned());
        assert_eq!(config.resolved_model(), "explicit-model-override");
    }
    let default = ModelExecutionConfig::default();
    assert_eq!(default.provider, ModelProvider::AnthropicClaudeCode);
    assert_eq!(default.resolved_model(), "claude-opus-5");
}

#[test]
fn gemini_aliases_select_the_antigravity_provider() {
    for alias in [
        "gemini",
        "gemini3.7",
        "gemini-3.7-flash",
        "gemini3.8",
        "gemini-3.8-flash",
        "GEMINI-3.8-FLASH",
    ] {
        assert_eq!(
            ModelProvider::from_str_name(alias),
            ModelProvider::Antigravity
        );
    }
}

#[test]
fn provider_labels_do_not_claim_a_fixed_gemini_model() {
    assert_eq!(
        ModelProvider::Antigravity.display_name(),
        "Google Antigravity Subscription (High Effort)"
    );
    assert_eq!(
        ModelProvider::SubscriptionEnsemble.display_name(),
        "Subscription Ensemble (Claude route)"
    );
}
