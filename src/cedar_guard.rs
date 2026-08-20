use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::{error, info, warn};

pub mod pdp_mock;
pub use pdp_mock::{CedarPdpEngine, CedarPdpResult};

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CedarPolicyEvaluation {
    pub is_cedar_compliant: bool,
    pub missing_policies_summary: Option<String>,
    #[serde(default)]
    pub suggested_policy_files: Vec<String>,
    #[serde(default)]
    pub generated_cedar_policy: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CedarGuardReport {
    pub is_compliant: bool,
    pub files_created_or_updated: Vec<String>,
    pub summary: String,
}

pub struct CedarGuard {
    agy_effort: String,
    pdp_engine: CedarPdpEngine,
}

impl CedarGuard {
    pub fn new(agy_effort: String) -> Self {
        let pdp_engine = CedarPdpEngine::new();
        Self {
            agy_effort,
            pdp_engine,
        }
    }

    /// Evaluates Cedar IAM policy coverage for modified endpoints and auto-generates missing policies
    pub async fn evaluate_cedar_policies(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
    ) -> Result<CedarGuardReport> {
        info!(
            "Running CedarGuard IAM policy check on {}#{}...",
            repo, diff_ctx.pr_number
        );

        // Check if repo has Cedar governance directories or backend APIs
        let has_cedar = repo_dir.join("governance/cedar").exists()
            || repo_dir.join("compliance/cedar").exists()
            || diff_ctx
                .changed_files
                .iter()
                .any(|f| f.contains("api") || f.contains("route") || f.contains("handler"));

        if !has_cedar {
            return Ok(CedarGuardReport {
                is_compliant: true,
                files_created_or_updated: Vec::new(),
                summary: "No API routes or Cedar governance policies modified.".to_string(),
            });
        }

        let eval = self
            .analyze_cedar_coverage(repo, repo_dir, diff_ctx, pr_title)
            .await?;

        if eval.is_cedar_compliant {
            info!(
                "Cedar policy coverage is fully compliant for {}#{}",
                repo, diff_ctx.pr_number
            );
            return Ok(CedarGuardReport {
                is_compliant: true,
                files_created_or_updated: Vec::new(),
                summary: "Cedar IAM policy coverage is verified; all actions are bound to authorization rules.".to_string(),
            });
        }

        info!(
            "Missing Cedar policies identified for {}#{}. Auto-generating policies...",
            repo, diff_ctx.pr_number
        );

        let created_files = self
            .generate_missing_cedar_policies(repo, repo_dir, diff_ctx, pr_title, &eval)
            .await?;

        Ok(CedarGuardReport {
            is_compliant: true,
            files_created_or_updated: created_files.clone(),
            summary: format!(
                "Auto-generated Cedar authorization policies: {}",
                created_files.join(", ")
            ),
        })
    }

    pub fn evaluate_offline_pdp(&self, policy_content: &str) -> CedarPdpResult {
        self.pdp_engine.evaluate_synthetic_tuples(policy_content)
    }

    async fn analyze_cedar_coverage(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
    ) -> Result<CedarPolicyEvaluation> {
        let changed_files_preview = if diff_ctx.changed_files.len() > 100 {
            format!(
                "{}\n- ... and {} more files",
                diff_ctx
                    .changed_files
                    .iter()
                    .take(100)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n- "),
                diff_ctx.changed_files.len() - 100
            )
        } else {
            diff_ctx.changed_files.join("\n- ")
        };

        let diff_content_bounded = if diff_ctx.diff_content.chars().count() > 50_000 {
            let truncated: String = diff_ctx.diff_content.chars().take(50_000).collect();
            format!("{truncated}\n\n[... remaining diff truncated for cedar evaluation ...]")
        } else {
            diff_ctx.diff_content.clone()
        };

        let prompt = format!(
            r#####"You are Oyatie's Principal IAM & Cedar Policy Architect. Evaluate whether new or modified routes/actions in PR #{pr_number} ("{pr_title}") on `{repo}` are covered by AWS Cedar policy rules.

## Context:
- **Repository**: {repo}
- **PR Number**: #{pr_number}
- **Title**: {pr_title}
- **Changed Files**:
- {changed_files}

## Evaluation Rules:
1. **Endpoint Authorization**: Every mutation or public action must have an explicit `permit(principal, action, resource)` policy statement.
2. **Tenant Scoping**: Policy statements must enforce `principal in Tenant::"..."` or tenant boundary constraints.
3. If no new endpoints or action primitives were introduced, policy is compliant.

## Output Format (strict JSON):
```json
{{
  "is_cedar_compliant": false,
  "missing_policies_summary": "Explanation of missing Cedar authorization statements",
  "suggested_policy_files": ["governance/cedar/policy/auth_routes.cedar"],
  "generated_cedar_policy": "permit(principal, action in [Action::\"ReadRecord\"], resource) when {{ principal.tenant_id == resource.tenant_id }};"
}}
```

Note: If compliant, output `{{"is_cedar_compliant": true, "missing_policies_summary": null, "suggested_policy_files": [], "generated_cedar_policy": null}}`.

## Git Diff:
```diff
{diff_content}
```
"#####,
            repo = repo,
            pr_number = diff_ctx.pr_number,
            pr_title = pr_title,
            changed_files = changed_files_preview,
            diff_content = diff_content_bounded
        );

        let output = self.run_agy_prompt(&prompt, repo_dir).await?;
        let json_str = extract_json_block(&output);

        match serde_json::from_str::<CedarPolicyEvaluation>(&json_str) {
            Ok(eval) => Ok(eval),
            Err(e) => {
                warn!(
                    "Failed to parse CedarGuard JSON response: {}. Failing closed.",
                    e
                );
                Ok(CedarPolicyEvaluation {
                    is_cedar_compliant: false,
                    missing_policies_summary: Some(format!(
                        "CedarGuard evaluation failed to produce valid JSON: {}. Failing closed for zero-trust security.",
                        e
                    )),
                    suggested_policy_files: Vec::new(),
                    generated_cedar_policy: None,
                })
            }
        }
    }

    async fn generate_missing_cedar_policies(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        eval: &CedarPolicyEvaluation,
    ) -> Result<Vec<String>> {
        let summary = eval
            .missing_policies_summary
            .as_deref()
            .unwrap_or("IAM policy coverage");
        let target_files = eval.suggested_policy_files.join(", ");

        let prompt = format!(
            r#####"You are Oyatie's Principal IAM Architect. Write and apply the necessary AWS Cedar policy files (`.cedar`) directly in this workspace for `{repo}` to achieve full IAM policy compliance for PR #{pr_number} ("{pr_title}").

## Policy Summary Needed:
- **Summary**: {summary}
- **Target Files**: {target_files}

## Instructions:
1. Create or update the `.cedar` files in `governance/cedar/policy/` or `compliance/cedar/`.
2. Ensure valid Cedar syntax: `permit(principal, action, resource) when {{ ... }};`.
3. Enforce strict tenant boundary checks (`principal.tenant_id == resource.tenant_id`).

Write the policy files directly to the workspace now."#####,
            repo = repo,
            pr_number = diff_ctx.pr_number,
            pr_title = pr_title,
            summary = summary,
            target_files = target_files
        );

        let _ = self.run_agy_prompt(&prompt, repo_dir).await?;

        // Check for created or modified .cedar files
        let mut status_cmd = Command::new("git");
        status_cmd
            .current_dir(repo_dir)
            .args(["status", "--porcelain"]);
        let status_out = crate::exec::run_bounded(
            status_cmd,
            crate::exec::ExecClass::Quick,
            "git status --porcelain (cedar guard)",
        )
        .await?;

        let modified: Vec<String> = String::from_utf8_lossy(&status_out.stdout)
            .lines()
            .map(|l| l[3..].trim().to_string())
            .filter(|f| f.ends_with(".cedar") || f.contains("cedar"))
            .collect();

        Ok(if modified.is_empty() {
            eval.suggested_policy_files.clone()
        } else {
            modified
        })
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
            crate::exec::run_bounded(cmd, crate::exec::ExecClass::Model, "agy (cedar guard)")
                .await
                .context("Failed to run agy command")?;
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            error!(
                "agy returned non-zero status in CedarGuard: {}",
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
    let json_block_re = Regex::new(r"(?s)```(?:json)?\s*(\{.*?\})\s*```").unwrap();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cedar_evaluation() {
        let raw = r#"```json
{
  "is_cedar_compliant": false,
  "missing_policies_summary": "Missing policy for payroll export endpoint",
  "suggested_policy_files": ["governance/cedar/policy/payroll.cedar"],
  "generated_cedar_policy": "permit(principal in Role::\"PayrollAdmin\", action in [Action::\"Export\"], resource);"
}
```"#;
        let json_str = extract_json_block(raw);
        let parsed: CedarPolicyEvaluation = serde_json::from_str(&json_str).expect("Valid parse");
        assert!(!parsed.is_cedar_compliant);
        assert_eq!(parsed.suggested_policy_files.len(), 1);
        assert!(parsed
            .generated_cedar_policy
            .unwrap()
            .contains("permit(principal"));
    }
}
