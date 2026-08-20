use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::{error, info, warn};

use crate::github::GitHubClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiTriageDiagnosis {
    pub failure_category: String, // "COMPILATION", "TEST_PANIC", "INFRASTRUCTURE", "TIMING_FLAKE", "LINT_TYPE"
    pub root_cause: String,
    pub culprit_file_and_line: Option<String>,
    pub actionable_remediation: String,
    pub formatted_markdown: String,
}

pub struct CiTriager {
    #[allow(dead_code)]
    github_client: std::sync::Arc<GitHubClient>,
    agy_effort: String,
}

impl CiTriager {
    pub fn new(github_client: std::sync::Arc<GitHubClient>, agy_effort: String) -> Self {
        Self {
            github_client,
            agy_effort,
        }
    }

    /// Triages a failed CI workflow run on main or dev branch
    pub async fn triage_workflow_run(
        &self,
        repo: &str,
        run_id: u64,
        branch: &str,
        commit_sha: &str,
        workflow_name: &str,
        repo_dir: &Path,
    ) -> Result<CiTriageDiagnosis> {
        info!(
            "Triaging failed workflow run #{} ('{}') on {}/{} (commit: {})...",
            run_id, workflow_name, repo, branch, commit_sha
        );

        // Fetch failed logs using `gh run view --log-failed`
        let failed_logs = self.fetch_failed_run_logs(repo, run_id).await?;

        if failed_logs.trim().is_empty() {
            warn!("No failed logs returned for run #{}", run_id);
        }

        // Analyze root cause with Antigravity
        let diagnosis = self
            .analyze_failure_logs(
                repo,
                run_id,
                branch,
                commit_sha,
                workflow_name,
                &failed_logs,
                repo_dir,
            )
            .await?;

        // Post triage diagnostic issue / comment
        self.publish_triage_report(repo, run_id, branch, &diagnosis)
            .await?;

        Ok(diagnosis)
    }

    async fn fetch_failed_run_logs(&self, repo: &str, run_id: u64) -> Result<String> {
        let mut logs_cmd = Command::new("gh");
        logs_cmd.args([
            "run",
            "view",
            &run_id.to_string(),
            "--repo",
            repo,
            "--log-failed",
        ]);
        let output = crate::exec::run_bounded(
            logs_cmd,
            crate::exec::ExecClass::Api,
            "gh run view --log-failed",
        )
        .await
        .context("Failed to execute gh run view --log-failed")?;

        let logs = if output.status.success() {
            String::from_utf8_lossy(&output.stdout).to_string()
        } else {
            String::from_utf8_lossy(&output.stderr).to_string()
        };

        // Cap the log snippet to stay within efficient model context.
        //
        // This previously sliced `logs[logs.len() - 20000..]`, a BYTE offset into
        // a UTF-8 string. `logs` comes from String::from_utf8_lossy over CI
        // output, which routinely contains multibyte characters -- cargo and
        // clippy emit checkmarks, arrows and box-drawing glyphs. Whenever that
        // offset landed mid-character, the slice panicked and killed the spawned
        // triage task. Reachable from any trunk CI failure with a log over 20 KB.
        Ok(tail_chars(&logs, 20_000))
    }

    #[allow(clippy::too_many_arguments)]
    async fn analyze_failure_logs(
        &self,
        repo: &str,
        run_id: u64,
        branch: &str,
        commit_sha: &str,
        workflow_name: &str,
        logs: &str,
        working_dir: &Path,
    ) -> Result<CiTriageDiagnosis> {
        let prompt = format!(
            r#####"You are Antigravity's Principal Infrastructure & Trunk Reliability Engineer. Conduct an automated Root Cause Diagnosis for a failed CI workflow on `{repo}`.

## Failure Context:
- **Repository**: {repo}
- **Branch**: {branch}
- **Workflow**: {workflow_name}
- **Workflow Run ID**: #{run_id}
- **Commit SHA**: {commit_sha}

## Failed Step Logs:
```text
{logs}
```

## Instructions:
1. Identify the exact error: compilation error, panicking test assertion, timing/flake hazard, network timeout, or infrastructure crash.
2. Pinpoint the culprit file and line number if visible in the stack trace.
3. Formulate clear, actionable remediation steps.

## Output Format:
Output strictly valid JSON matching this schema:
```json
{{
  "failure_category": "COMPILATION | TEST_PANIC | TIMING_FLAKE | INFRASTRUCTURE | LINT_TYPE",
  "root_cause": "Concise 1-2 sentence explanation of the failure mechanism",
  "culprit_file_and_line": "path/to/file.rs:42",
  "actionable_remediation": "Clear instructions for fixing the problem",
  "formatted_markdown": "### 🚨 Trunk CI Failure Diagnostic: {workflow_name} on `{branch}`\n\n| Attribute | Details |\n|---|---|\n| **Category** | ... |\n| **Culprit File** | ... |\n| **Root Cause** | ... |\n\n#### 🔍 Diagnostic Breakdown\n...\n\n#### 🛠️ Recommended Remediation\n..."
}}
```
"#####,
            repo = repo,
            branch = branch,
            workflow_name = workflow_name,
            run_id = run_id,
            commit_sha = commit_sha,
            logs = logs
        );

        let output = self.run_agy_prompt(&prompt, working_dir).await?;
        let json_candidate = extract_json_block(&output);

        match serde_json::from_str::<CiTriageDiagnosis>(&json_candidate) {
            Ok(diag) => Ok(diag),
            Err(e) => {
                warn!(
                    "Failed to parse CI triage JSON: {}. Building fallback diagnosis.",
                    e
                );
                Ok(CiTriageDiagnosis {
                    failure_category: "UNSPECIFIED".to_string(),
                    root_cause: "Workflow run failed on trunk".to_string(),
                    culprit_file_and_line: None,
                    actionable_remediation: "Inspect workflow failure logs for details".to_string(),
                    formatted_markdown: format!(
                        "### 🚨 Trunk CI Failure on `{}` (Run #{})\n\n**Logs Snippet:**\n```text\n{}\n```",
                        branch, run_id, logs.lines().take(30).collect::<Vec<_>>().join("\n")
                    ),
                })
            }
        }
    }

    async fn publish_triage_report(
        &self,
        repo: &str,
        run_id: u64,
        branch: &str,
        diag: &CiTriageDiagnosis,
    ) -> Result<()> {
        info!(
            "Publishing trunk CI triage diagnostic for {} (Run #{})",
            repo, run_id
        );

        let title = format!(
            "🚨 Trunk CI Failure on `{}`: Run #{} ({})",
            branch, run_id, diag.failure_category
        );
        let body = format!(
            "{}\n\n*Run URL: https://github.com/{}/actions/runs/{}*\n\n---\n*🤖 [Triaged] by Oyatie Anvil*",
            diag.formatted_markdown, repo, run_id
        );

        // Open an issue on GitHub to alert maintainers
        let mut cmd = Command::new("gh");
        cmd.args([
            "issue", "create", "--repo", repo, "--title", &title, "--body", &body,
        ]);

        let out = crate::exec::run_bounded(
            cmd,
            crate::exec::ExecClass::Api,
            "gh issue create (ci triage report)",
        )
        .await?;
        if out.status.success() {
            let issue_url = String::from_utf8_lossy(&out.stdout).trim().to_string();
            info!("Successfully created triage issue: {}", issue_url);
        } else {
            let err = String::from_utf8_lossy(&out.stderr);
            warn!("Could not create triage issue: {}", err);
        }

        Ok(())
    }

    async fn run_agy_prompt(&self, prompt: &str, working_dir: &Path) -> Result<String> {
        let mut cmd = Command::new("agy");
        cmd.args([
            "--print",
            prompt,
            "--effort",
            &self.agy_effort,
            "--dangerously-skip-permissions",
        ]);
        cmd.current_dir(working_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output =
            crate::exec::run_bounded(cmd, crate::exec::ExecClass::Model, "agy (ci triager)")
                .await
                .context("Failed to run agy command")?;

        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            error!(
                "agy returned non-zero status in CiTriager: {}",
                output.status
            );
            warn!("agy stderr: {}", stderr_str);
            if stdout_str.trim().is_empty() {
                bail!("agy failed with code {}: {}", output.status, stderr_str);
            }
        }

        Ok(stdout_str)
    }
}

fn extract_json_block(text: &str) -> String {
    let json_block_re = regex::Regex::new(r"(?s)```(?:json)?\s*(\{.*?\})\s*```").unwrap();
    if let Some(caps) = json_block_re.captures(text) {
        if let Some(m) = caps.get(1) {
            return m.as_str().to_string();
        }
    }

    if let (Some(first), Some(last)) = (text.find('{'), text.rfind('}')) {
        if first < last {
            return text[first..=last].to_string();
        }
    }

    text.to_string()
}

/// Returns the last `max_chars` characters, never splitting a UTF-8 character.
///
/// Operates on character boundaries rather than byte offsets.
fn tail_chars(s: &str, max_chars: usize) -> String {
    let total = s.chars().count();
    if total <= max_chars {
        return s.to_string();
    }
    // Byte index of the character that starts the tail.
    let start = s
        .char_indices()
        .nth(total - max_chars)
        .map(|(i, _)| i)
        .unwrap_or(0);
    s[start..].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(parsed.failure_category, "COMPILATION");
        assert_eq!(
            parsed.culprit_file_and_line.as_deref(),
            Some("src/models.rs:54")
        );
        assert!(parsed.root_cause.contains("Missing trait bound"));
    }
}

#[cfg(test)]
mod tail_tests {
    use super::tail_chars;

    #[test]
    fn does_not_panic_on_multibyte_boundaries() {
        // Byte-slicing this at an arbitrary offset panics; char-slicing must not.
        let logs = "✓ ok → next ─────".repeat(3000);
        assert!(logs.len() > 20_000, "fixture must exceed the cap in BYTES");
        let out = tail_chars(&logs, 20_000);
        assert_eq!(out.chars().count(), 20_000);
        assert!(logs.ends_with(&out));
    }

    #[test]
    fn returns_input_unchanged_when_short() {
        assert_eq!(tail_chars("short", 20_000), "short");
        assert_eq!(tail_chars("", 10), "");
    }

    #[test]
    fn keeps_the_tail_not_the_head() {
        assert_eq!(tail_chars("abcdef", 3), "def");
    }

    #[test]
    fn handles_a_cap_landing_exactly_on_a_multibyte_char() {
        let s = "aaa日本語";
        assert_eq!(tail_chars(s, 3), "日本語");
        assert_eq!(tail_chars(s, 4), "a日本語");
    }
}
