use super::*;

#[test]
fn ci_prompt_uses_trusted_unknown_for_an_omitted_commit_sha() {
    for commit_sha in [None, Some("")] {
        let prompt = build_ci_triage_prompt(
            "oyatie/anvil",
            42,
            "main",
            commit_sha,
            "presubmit",
            "error: fixture failure",
        )
        .expect("an omitted commit SHA is valid metadata absence");
        assert!(!prompt.is_empty());
    }
}

#[test]
fn ci_prompt_still_rejects_a_malformed_nonempty_commit_sha() {
    let error = match build_ci_triage_prompt(
        "oyatie/anvil",
        42,
        "main",
        Some("not-a-sha"),
        "presubmit",
        "error: fixture failure",
    ) {
        Ok(_) => panic!("nonempty commit metadata must remain strictly validated"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("invalid commit SHA"));
}

#[test]
fn test_parse_ci_triage_diagnosis() {
    let raw = r#####"```json
{
  "failure_category": "COMPILATION",
  "root_cause": "Missing trait bound `Serialize` on struct AppPayload",
  "culprit_file_and_line": "src/models.rs:54",
  "actionable_remediation": "Add #[derive(Serialize)] to AppPayload",
  "formatted_markdown": "### Trunk CI Failure Diagnostic..."
}
```"#####;
    let json_str = extract_json_block(raw);
    let parsed: CiTriageDiagnosis = serde_json::from_str(&json_str).expect("Valid parse");
    assert!(matches!(
        parsed.failure_category,
        CiFailureCategory::Compilation
    ));
    assert_eq!(
        parsed.culprit_file_and_line.as_deref(),
        Some("src/models.rs:54")
    );
    assert!(parsed.root_cause.contains("Missing trait bound"));
}

#[test]
fn fallback_bounds_one_huge_line_and_keeps_its_diagnostic_tail() {
    let logs = format!(
        "HEAD_SENTINEL{}</pre>```\nFINAL_DIAGNOSTIC_🦀",
        "界".repeat(MAX_CI_FALLBACK_DIAGNOSTIC_BYTES * 2)
    );
    let diagnosis = fallback_diagnosis(42, &logs);
    let (escaped, selected_bytes) = escaped_log_tail(&logs);

    assert!(escaped.len() <= MAX_CI_FALLBACK_DIAGNOSTIC_BYTES);
    assert!(selected_bytes <= MAX_CI_FALLBACK_DIAGNOSTIC_BYTES);
    assert!(!diagnosis.formatted_markdown.contains("HEAD_SENTINEL"));
    assert!(diagnosis.formatted_markdown.contains("FINAL_DIAGNOSTIC_🦀"));
    assert!(diagnosis.formatted_markdown.contains("&lt;/pre&gt;"));
    assert!(
        diagnosis
            .formatted_markdown
            .contains(&logs.len().to_string())
    );
}

#[test]
fn final_issue_body_caps_model_markdown_and_preserves_trusted_suffix() {
    let markdown = format!("MODEL_HEAD{}MODEL_TAIL", "界".repeat(100_000));
    let body = publication::build_issue_body("oyatie/console", 42, &markdown)
        .expect("trusted suffix fits");

    assert!(body.len() <= publication::MAX_CI_ISSUE_BODY_BYTES);
    assert!(body.starts_with("MODEL_HEAD"));
    assert!(!body.contains("MODEL_TAIL"));
    assert!(body.contains(&markdown.len().to_string()));
    assert!(body.ends_with("*🤖 [Triaged] by Oyatie Anvil*"));
    assert!(body.contains("https://github.com/oyatie/console/actions/runs/42"));
}

#[cfg(unix)]
#[tokio::test]
async fn issue_body_is_exact_on_stdin_and_absent_from_argv() {
    use std::os::unix::fs::PermissionsExt;

    let markdown = "MODEL_BODY_SENTINEL";
    let expected = publication::build_issue_body("oyatie/console", 42, markdown).unwrap();
    let scratch = tempfile::tempdir().expect("capture directory");
    let executable = scratch.path().join("gh");
    std::fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" >&2\nexec /bin/cat\n",
    )
    .expect("capture executable");
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
        .expect("capture executable permissions");
    let capture = tokio::process::Command::new(executable);
    let output = publication::create_issue(capture, "oyatie/console", 42, markdown)
        .await
        .expect("capture transport runs");
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    let argv = String::from_utf8(output.stderr).unwrap();
    assert!(argv.contains("--body-file\n-\n"));
    assert!(!argv.contains("MODEL_BODY_SENTINEL"));
}
