use crate::model_prompt::{HarnessText, ModelPrompt};
use crate::reviewer::untrusted::{Untrusted, UntrustedLabel};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{error, info, warn};

use crate::github::GitHubClient;

mod publication;

/// Maximum escaped log bytes included in the human-facing fallback report.
/// This is independent of the model prompt's CI-log cap: both retain the tail,
/// but the fallback is published directly when no model JSON was obtained.
const MAX_CI_FALLBACK_DIAGNOSTIC_BYTES: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CiFailureCategory {
    Compilation,
    TestPanic,
    Infrastructure,
    TimingFlake,
    LintType,
    Unspecified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiTriageDiagnosis {
    pub failure_category: CiFailureCategory,
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
        self.triage_workflow_run_with_optional_sha(
            repo,
            run_id,
            branch,
            (!commit_sha.is_empty()).then_some(commit_sha),
            workflow_name,
            repo_dir,
        )
        .await
    }

    pub(crate) async fn triage_workflow_run_with_optional_sha(
        &self,
        repo: &str,
        run_id: u64,
        branch: &str,
        commit_sha: Option<&str>,
        workflow_name: &str,
        repo_dir: &Path,
    ) -> Result<CiTriageDiagnosis> {
        info!(
            "Triaging failed workflow run #{} ('{}') on {}/{} (commit: {})...",
            run_id,
            workflow_name,
            repo,
            branch,
            commit_sha
                .filter(|sha| !sha.is_empty())
                .unwrap_or("unknown")
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
        self.publish_triage_report(repo, run_id, &diagnosis).await?;

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

        // `Untrusted::CiLogs` owns the sole cap and deliberately keeps the
        // diagnostic tail. Returning the full source here preserves the real
        // measured length in its truncation declaration.
        Ok(logs)
    }

    #[allow(clippy::too_many_arguments)]
    async fn analyze_failure_logs(
        &self,
        repo: &str,
        run_id: u64,
        branch: &str,
        commit_sha: Option<&str>,
        workflow_name: &str,
        logs: &str,
        working_dir: &Path,
    ) -> Result<CiTriageDiagnosis> {
        let prompt = build_ci_triage_prompt(repo, run_id, branch, commit_sha, workflow_name, logs)?;

        let output = self.run_agy_prompt(&prompt, working_dir).await?;
        let json_candidate = extract_json_block(&output);

        match serde_json::from_str::<CiTriageDiagnosis>(&json_candidate) {
            Ok(diag) => Ok(diag),
            Err(e) => {
                warn!(
                    "Failed to parse CI triage JSON: {}. Building fallback diagnosis.",
                    e
                );
                Ok(fallback_diagnosis(run_id, logs))
            }
        }
    }

    async fn publish_triage_report(
        &self,
        repo: &str,
        run_id: u64,
        diag: &CiTriageDiagnosis,
    ) -> Result<()> {
        info!(
            "Publishing trunk CI triage diagnostic for {} (Run #{})",
            repo, run_id
        );

        let out =
            publication::create_issue(crate::exec::gh(), repo, run_id, &diag.formatted_markdown)
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

    async fn run_agy_prompt(&self, prompt: &ModelPrompt, working_dir: &Path) -> Result<String> {
        let budget = crate::exec::ExecClass::Model.timeout();
        let cmd = crate::exec::agy_agent(
            &crate::exec::Posture::in_workspace(working_dir),
            &self.agy_effort,
            budget,
            None,
        )?;

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

fn build_ci_triage_prompt(
    repo: &str,
    run_id: u64,
    branch: &str,
    commit_sha: Option<&str>,
    workflow_name: &str,
    logs: &str,
) -> Result<ModelPrompt> {
    let mut prompt = ModelPrompt::builder();
    prompt.push_harness(HarnessText::CiPreambleAndRepository);
    prompt.push_repository(repo)?;
    prompt
        .push_harness(HarnessText::CiRunId)
        .push_u64(run_id)
        .push_harness(HarnessText::CiCommitSha);
    if let Some(commit_sha) = commit_sha.filter(|sha| !sha.is_empty()) {
        prompt.push_commit_sha(commit_sha)?;
    } else {
        prompt.push_harness(HarnessText::CiUnknownCommitSha);
    }
    prompt
        .push_harness(HarnessText::CiMetadataEnd)
        .push_untrusted(Untrusted::new(UntrustedLabel::BranchName, branch))
        .push_untrusted(Untrusted::new(UntrustedLabel::WorkflowName, workflow_name))
        .push_untrusted(Untrusted::new(UntrustedLabel::CiLogs, logs))
        .push_harness(HarnessText::CiResponseContract);
    prompt.finish()
}

/// HTML-escapes the longest UTF-8 suffix whose escaped representation fits the
/// fallback byte budget. Selecting before materialising bounds allocation even
/// for one marker-stuffed, attacker-controlled line.
fn escaped_log_tail(logs: &str) -> (String, usize) {
    let escaped_len = |ch: char| match ch {
        '&' => 5,
        '<' | '>' => 4,
        _ => ch.len_utf8(),
    };
    let mut start = logs.len();
    let mut rendered_len = 0usize;
    for (index, ch) in logs.char_indices().rev() {
        let next = escaped_len(ch);
        if rendered_len + next > MAX_CI_FALLBACK_DIAGNOSTIC_BYTES {
            break;
        }
        rendered_len += next;
        start = index;
    }

    let selected = &logs[start..];
    let mut escaped = String::with_capacity(rendered_len);
    for ch in selected.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            _ => escaped.push(ch),
        }
    }
    (escaped, selected.len())
}

fn fallback_diagnosis(run_id: u64, logs: &str) -> CiTriageDiagnosis {
    let (tail, selected_bytes) = escaped_log_tail(logs);
    CiTriageDiagnosis {
        failure_category: CiFailureCategory::Unspecified,
        root_cause: "Workflow run failed on trunk".to_string(),
        culprit_file_and_line: None,
        actionable_remediation: "Inspect workflow failure logs for details".to_string(),
        formatted_markdown: format!(
            "### 🚨 Trunk CI Failure (Run #{run_id})\n\n\
             **Logs Tail:** final {selected_bytes} bytes of {} original bytes\n\n\
             <pre>{tail}</pre>",
            logs.len()
        ),
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
mod tests;
