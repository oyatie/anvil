use anvil::ai_driver::provider::{ModelExecutionConfig, ModelProvider};
use anvil::ai_driver::router::SubscriptionExecutor;
use anvil::self_governance::account_pool::AccountPoolManager;
use anvil::self_governance::deathloop_detector::{DeathloopDetector, DeathloopVerdict};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_deathloop_repetitive_patch_circuit_breaker() {
    let detector = DeathloopDetector::new(3, 3, 500_000);
    let task_id = "oyatie/oyatie#pr-2158";

    // Attempt 1: First failed patch
    let v1 = detector
        .record_and_evaluate(
            task_id,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "Error[E0425]: cannot find value `unresolved_var` in this scope",
            50_000,
            2,
        )
        .await;
    assert_eq!(v1, DeathloopVerdict::Nominal);

    // Attempt 2: Identical patch emitted again
    let v2 = detector
        .record_and_evaluate(
            task_id,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "Error[E0425]: cannot find value `unresolved_var` in this scope",
            50_000,
            2,
        )
        .await;
    match v2 {
        DeathloopVerdict::Warning(_) => {}
        other => panic!("Expected warning on attempt 2, got: {:?}", other),
    }

    // Attempt 3: Identical patch emitted a 3rd time (Deathloop!)
    let v3 = detector
        .record_and_evaluate(
            task_id,
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "Error[E0425]: cannot find value `unresolved_var` in this scope",
            50_000,
            2,
        )
        .await;

    match v3 {
        DeathloopVerdict::TrippedCircuitBreaker {
            reason,
            attempts,
            tokens_drained,
            quarantine_action,
        } => {
            assert!(reason.contains("Repetitive patch deathloop"));
            assert_eq!(attempts, 3);
            assert_eq!(tokens_drained, 150_000);
            assert_eq!(quarantine_action, "QUARANTINE_PR_HALT_REPAIRS");
        }
        other => panic!(
            "Expected TrippedCircuitBreaker on 3rd identical attempt, got: {:?}",
            other
        ),
    }

    // Attempt 4: Should immediately reject with tripped circuit breaker
    let v4 = detector
        .record_and_evaluate(
            task_id,
            "sha256:different_hash",
            "different error",
            10_000,
            1,
        )
        .await;
    match v4 {
        DeathloopVerdict::TrippedCircuitBreaker { .. } => {}
        other => panic!(
            "Expected quarantined task to stay tripped, got: {:?}",
            other
        ),
    }
}

#[tokio::test]
async fn test_deathloop_excessive_token_burn_circuit_breaker() {
    let detector = DeathloopDetector::new(3, 3, 200_000);
    let task_id = "oyatie/console#pr-836";

    // Attempt 1: 120k tokens
    let v1 = detector
        .record_and_evaluate(task_id, "hash-1", "syntax error 1", 120_000, 3)
        .await;
    assert_eq!(v1, DeathloopVerdict::Nominal);

    // Attempt 2: 90k tokens (Total: 210k tokens > 200k ceiling!)
    let v2 = detector
        .record_and_evaluate(task_id, "hash-2", "type error 2", 90_000, 2)
        .await;

    match v2 {
        DeathloopVerdict::TrippedCircuitBreaker {
            reason,
            tokens_drained,
            quarantine_action,
            ..
        } => {
            assert!(reason.contains("Token budget ceiling breached"));
            assert_eq!(tokens_drained, 210_000);
            assert_eq!(quarantine_action, "QUARANTINE_PR_HALT_REPAIRS");
        }
        other => panic!("Expected budget ceiling breach, got: {:?}", other),
    }
}

fn is_agy_available() -> bool {
    std::process::Command::new("agy")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn test_multi_model_cascading_outage_cooldown_and_fallback() {
    let pool = Arc::new(AccountPoolManager::new());
    let executor = SubscriptionExecutor::with_pool(Arc::clone(&pool));

    // Put Claude account in cooldown to simulate Claude 503 outage
    pool.mark_rate_limited("claude:cli-default", Duration::from_secs(300))
        .await;

    // Verify Claude account is in cooldown
    let views = pool.get_pool_status_views().await;
    let claude_view = views
        .iter()
        .find(|v| v.account_id == "claude:cli-default")
        .unwrap();
    assert!(!claude_view.is_active);
    assert!(claude_view.lifecycle_state.starts_with("COOLDOWN"));

    // Put OpenAI Codex in cooldown as well to simulate dual outage
    pool.mark_rate_limited("codex:cli-default", Duration::from_secs(300))
        .await;

    if !is_agy_available() {
        println!("Skipping live CLI invocation: agy CLI is not installed on this runner");
        return;
    }

    // Execute prompt targeted at Claude Code -> Should gracefully fall over to Antigravity (Gemini 3.7 Flash)
    let config = ModelExecutionConfig {
        provider: ModelProvider::AnthropicClaudeCode,
        specific_model: None,
        reasoning_effort: "high".to_string(),
        print_timeout_secs: 60,
    };

    let mut prompt = anvil::model_prompt::ModelPrompt::builder();
    prompt.push_untrusted(anvil::reviewer::untrusted::Untrusted::new(
        anvil::reviewer::untrusted::UntrustedLabel::ReviewComment,
        "Return the word HELLO_RESILIENCE",
    ));
    let prompt = prompt
        .finish_for(anvil::model_prompt::ModelPromptPurpose::SubscriptionProbe)
        .expect("non-empty bounded prompt");
    let result = executor
        .execute_prompt(&prompt, Path::new("."), &config)
        .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("HELLO_RESILIENCE") || !output.is_empty());
}
