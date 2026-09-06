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
fn failed_gh_output_is_not_relabelled_as_ci_logs() {
    let error = decode_failed_logs(false, Vec::new(), b"HTTP 503 from API")
        .expect_err("CLI failure is absent CI evidence");
    assert!(
        error.to_string().contains("no CI-log evidence"),
        "{error:#}"
    );
}

#[test]
fn ci_logs_must_be_lossless_utf8() {
    let error = decode_failed_logs(true, vec![0xff], b"")
        .expect_err("lossy logs cannot become the triage corpus");
    assert!(error.to_string().contains("non-UTF-8"), "{error:#}");
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

#[test]
fn issue_body_is_exact_on_stdin_and_absent_from_argv() {
    let markdown = "MODEL_BODY_SENTINEL";
    let expected = publication::build_issue_body("oyatie/console", 42, markdown).unwrap();
    let command = crate::exec::gh();
    let (command, stdin) =
        publication::prepare_issue(command, "oyatie/console", 42, markdown).unwrap();
    assert_eq!(stdin, expected);
    let argv = command
        .as_std()
        .get_args()
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(argv.windows(2).any(|args| args == ["--body-file", "-"]));
    assert!(!argv.iter().any(|argument| argument.contains(markdown)));
}
