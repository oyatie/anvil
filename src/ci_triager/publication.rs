//! Bounded CI issue publication with the body delivered only on STDIN.

use anyhow::{Result, bail};
use std::process::Output;
use tokio::process::Command;

/// Below GitHub's issue-body limit, expressed as rendered UTF-8 bytes.
pub(super) const MAX_CI_ISSUE_BODY_BYTES: usize = 60_000;

fn utf8_prefix(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        return value;
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}

pub(super) fn build_issue_body(repo: &str, run_id: u64, markdown: &str) -> Result<String> {
    let suffix = format!(
        "\n\n*Run URL: https://github.com/{repo}/actions/runs/{run_id}*\n\n---\n\
         *🤖 [Triaged] by Oyatie Anvil*"
    );
    if suffix.len() >= MAX_CI_ISSUE_BODY_BYTES {
        bail!("trusted CI issue suffix exceeds the complete issue-body budget");
    }
    if markdown.len() + suffix.len() <= MAX_CI_ISSUE_BODY_BYTES {
        return Ok(format!("{markdown}{suffix}"));
    }

    let notice = format!(
        "\n\n_[Diagnostic truncated from {} original UTF-8 bytes to fit the issue-body limit.]_",
        markdown.len()
    );
    let available = MAX_CI_ISSUE_BODY_BYTES
        .checked_sub(suffix.len() + notice.len())
        .ok_or_else(|| anyhow::anyhow!("trusted CI issue suffix leaves no diagnostic budget"))?;
    let diagnostic = utf8_prefix(markdown, available);
    let body = format!("{diagnostic}{notice}{suffix}");
    debug_assert!(body.len() <= MAX_CI_ISSUE_BODY_BYTES);
    Ok(body)
}

/// Adds the finite issue-create argv and delivers the bounded body on STDIN.
pub(super) async fn create_issue(
    mut cmd: Command,
    repo: &str,
    run_id: u64,
    markdown: &str,
) -> Result<Output> {
    let title = format!("🚨 Trunk CI Failure: Run #{run_id}");
    let body = build_issue_body(repo, run_id, markdown)?;
    cmd.args([
        "issue",
        "create",
        "--repo",
        repo,
        "--title",
        &title,
        "--body-file",
        "-",
    ]);
    crate::exec::run_bounded_with_stdin(
        cmd,
        &body,
        crate::exec::ExecClass::Api.timeout(),
        "gh issue create (ci triage report)",
    )
    .await
}
