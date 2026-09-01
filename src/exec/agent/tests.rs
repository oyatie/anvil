use super::*;

fn daemon_environment() -> Vec<(String, String)> {
    vec![
        ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
        (
            "GITHUB_WEBHOOK_SECRET".into(),
            "anvil-test-secret-2f9c".into(),
        ),
        ("GITHUB_TOKEN".into(), "ghp_anvil_test_token".into()),
        ("SSH_AUTH_SOCK".into(), "/tmp/anvil-test-agent.sock".into()),
    ]
}

fn explicit_environment(command: &AgentCommand) -> std::collections::BTreeMap<String, String> {
    command
        .as_std()
        .get_envs()
        .filter_map(|(name, value)| {
            value.map(|value| {
                (
                    name.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
        })
        .collect()
}

#[test]
fn a_daemon_secret_does_not_reach_a_model_turn() {
    let posture = Posture::in_workspace(std::env::temp_dir());
    let cmd = command_in("claude", &posture, Framing::Plain, daemon_environment());
    let seen = explicit_environment(&cmd);

    assert!(seen.contains_key("GH_CONFIG_DIR"));
    let leaked: Vec<&str> = seen
        .iter()
        .filter(|(_, value)| {
            [
                "anvil-test-secret-2f9c",
                "ghp_anvil_test_token",
                "agent.sock",
            ]
            .iter()
            .any(|secret| value.contains(secret))
        })
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(
        leaked.is_empty(),
        "secrets reached a model turn: {leaked:?}"
    );

    let allowed: std::collections::BTreeSet<&str> =
        INHERITED.iter().copied().chain(["GH_CONFIG_DIR"]).collect();
    let unexpected: Vec<&str> = seen
        .keys()
        .map(String::as_str)
        .filter(|name| !allowed.contains(name))
        .collect();
    assert!(
        unexpected.is_empty(),
        "unexpected environment: {unexpected:?}"
    );
    assert!(seen.contains_key("PATH"));
}

#[test]
fn a_leased_credential_reaches_only_its_turn() {
    let posture = Posture::in_workspace(std::env::temp_dir())
        .with_credential("GEMINI_API_KEY", "leased-for-this-turn");
    let cmd = command("claude", &posture, Framing::Plain);
    assert_eq!(
        explicit_environment(&cmd).get("GEMINI_API_KEY"),
        Some(&"leased-for-this-turn".to_string())
    );
}

#[test]
fn the_turn_runs_in_the_selected_workspace() {
    let dir = std::env::temp_dir().join("anvil-posture-cwd");
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let cmd = command("claude", &Posture::in_workspace(&dir), Framing::Plain);
    assert_eq!(cmd.as_std().get_current_dir(), Some(dir.as_path()));
}
