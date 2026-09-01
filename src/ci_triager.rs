use crate::reviewer::untrusted::{Untrusted, UntrustedLabel};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
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
        let mut logs_cmd = crate::exec::gh();
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

        // Returned whole. `Untrusted<CiLogs>` is the single truncation point,
        // and it keeps the trailing bytes.
        //
        // Capping here as well was two bounds in different units retaining
        // opposite ends: this kept the last 20,000 CHARS, then `cap_declaring`
        // kept the leading 20,000 BYTES of that. CI output is multibyte -- cargo
        // and clippy emit checkmarks, arrows and box-drawing glyphs -- so chars
        // exceeded bytes, the second cap fired, and what survived was the FRONT
        // of a tail: `test result: FAILED`, the panic and the exit code were
        // dropped from the prompt whose only job is to diagnose them.
        Ok(logs)
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

{logs}

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
            // Contributor-controlled despite looking like machine output: a
            // test the pull request adds prints whatever it likes.
            logs = Untrusted::new(UntrustedLabel::CiLogs, logs,).render()
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
                        branch,
                        run_id,
                        logs.lines().take(30).collect::<Vec<_>>().join("\n")
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
        let mut cmd = crate::exec::gh();
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
        let budget = crate::exec::ExecClass::Model.timeout();
        let mut cmd = crate::exec::agent("agy", &crate::exec::Posture::in_workspace(working_dir));
        crate::exec::turn::agy_turn(&mut cmd, &self.agy_effort, budget);

        let turn = crate::exec::turn::run(cmd, prompt, budget, "agy (ci triager)")
            .await
            .context("Failed to run agy command")?;

        if !turn.status.success() {
            error!("agy returned non-zero status in CiTriager: {}", turn.status);
            warn!("agy stderr: {}", turn.stderr);
        }

        turn.into_result()
    }
}

fn extract_json_block(text: &str) -> String {
    let json_block_re = regex::Regex::new(r"(?s)```(?:json)?\s*(\{.*?\})\s*```").unwrap();
    if let Some(caps) = json_block_re.captures(text)
        && let Some(m) = caps.get(1)
    {
        return m.as_str().to_string();
    }

    if let (Some(first), Some(last)) = (text.find('{'), text.rfind('}'))
        && first < last
    {
        return text[first..=last].to_string();
    }

    text.to_string()
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
