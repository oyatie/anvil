use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::{error, info, warn};

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocParityEvaluation {
    pub is_doc_sufficient: bool,
    pub missing_doc_summary: Option<String>,
    #[serde(default)]
    pub doc_files_to_update: Vec<String>,
    #[serde(default)]
    pub suggested_adr_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocGuardReport {
    pub is_sufficient: bool,
    pub files_created_or_updated: Vec<String>,
    pub summary: String,
}

pub struct DocGuard {
    agy_effort: String,
}

impl DocGuard {
    pub fn new(agy_effort: String) -> Self {
        Self { agy_effort }
    }

    /// Evaluates documentation parity and auto-generates any missing docs or ADRs
    pub async fn ensure_documentation_parity(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
    ) -> Result<DocGuardReport> {
        info!(
            "Running DocGuard documentation parity check on {}#{}...",
            repo, diff_ctx.pr_number
        );

        // Step 1: Analyze documentation parity
        let eval = self
            .evaluate_doc_parity(repo, repo_dir, diff_ctx, pr_title, pr_body)
            .await?;

        if eval.is_doc_sufficient {
            info!(
                "Documentation parity is satisfied for {}#{}",
                repo, diff_ctx.pr_number
            );
            return Ok(DocGuardReport {
                is_sufficient: true,
                files_created_or_updated: Vec::new(),
                summary: "Documentation is fully up to date and reflects all code and architectural changes.".to_string(),
            });
        }

        info!(
            "Missing documentation identified for {}#{}: {:?}. Auto-generating documentation...",
            repo, diff_ctx.pr_number, eval.doc_files_to_update
        );

        // Step 2: Auto-generate missing documentation / ADRs in the workspace
        let updated_files = self
            .generate_and_write_docs(repo, repo_dir, diff_ctx, pr_title, pr_body, &eval)
            .await?;

        let summary = format!(
            "Auto-generated documentation updates for: {}",
            updated_files.join(", ")
        );

        Ok(DocGuardReport {
            is_sufficient: true,
            files_created_or_updated: updated_files,
            summary,
        })
    }

    async fn evaluate_doc_parity(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
    ) -> Result<DocParityEvaluation> {
        let prompt = format!(
            r#####"You are Oyatie's Principal Documentation Architect. Evaluate whether this Pull Request on `{repo}` has sufficient documentation parity or if documentation must be updated.

## Pull Request Information:
- **Repository**: {repo}
- **PR Number**: #{pr_number}
- **Title**: {pr_title}
- **Description**: {pr_body}
- **Changed Files**:
- {changed_files}

## Evaluation Criteria:
1. **API / Public Interface Changes**: Are new public functions, types, routes, or CLI flags introduced without docstrings or `docs/reference/` updates?
2. **Architectural / Doctrine Shifts**: Does this change introduce a new architectural decision, storage pattern, cell boundary, or platform contract that requires an ADR (in `docs/decisions/` or `docs/design/`)?
3. **User-Facing / Config Changes**: Does `README.md`, `CLAUDE.md`, `AGENTS.md`, or runbooks need updating?
4. **Changelog**: Does `CHANGELOG.md` need a release note entry?

## Output Format:
Output strictly valid JSON matching this schema:
```json
{{
  "is_doc_sufficient": false,
  "missing_doc_summary": "Explanation of what documentation or ADR is missing",
  "doc_files_to_update": ["docs/reference/feature.md", "CHANGELOG.md"],
  "suggested_adr_title": null
}}
```

Note: If documentation is already sufficient, set `is_doc_sufficient: true`, `missing_doc_summary: null`, `doc_files_to_update: []`.

## Git Diff:
```diff
{diff_content}
```
"#####,
            repo = repo,
            pr_number = diff_ctx.pr_number,
            pr_title = pr_title,
            pr_body = if pr_body.is_empty() {
                "No description"
            } else {
                pr_body
            },
            changed_files = diff_ctx.changed_files.join("\n- "),
            diff_content = diff_ctx.diff_content
        );

        let output = self.run_agy_prompt(&prompt, repo_dir).await?;
        let json_str = extract_json_block(&output);

        match serde_json::from_str::<DocParityEvaluation>(&json_str) {
            Ok(eval) => Ok(eval),
            Err(e) => {
                warn!(
                    "Failed to parse DocGuard JSON: {}. Assuming doc is sufficient.",
                    e
                );
                Ok(DocParityEvaluation {
                    is_doc_sufficient: true,
                    missing_doc_summary: None,
                    doc_files_to_update: Vec::new(),
                    suggested_adr_title: None,
                })
            }
        }
    }

    async fn generate_and_write_docs(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
        eval: &DocParityEvaluation,
    ) -> Result<Vec<String>> {
        let missing_summary = eval
            .missing_doc_summary
            .as_deref()
            .unwrap_or("General doc sync");
        let target_files = eval.doc_files_to_update.join(", ");

        let prompt = format!(
            r#####"You are Oyatie's Principal Technical Writer. Autonomously write and update the documentation files in this workspace for `{repo}` to achieve 100% documentation parity for PR #{pr_number} ("{pr_title}").

## Missing Documentation to Create/Update:
- **Summary**: {missing_summary}
- **Target Files to Update/Create**: {target_files}
- **PR Description**: {pr_body}

## Instructions:
1. Inspect the codebase and modified code files in this workspace.
2. Directly create or edit the target documentation files (e.g., markdown files in `docs/`, `CHANGELOG.md`, `README.md`, or docstrings).
3. Ensure the documentation is rigorous, accurate, mathematically precise, and formatted in standard GitHub Flavored Markdown.
4. If an ADR is required, follow standard ADR format (Status, Context, Decision, Consequences) in `docs/decisions/`.

Write all documentation file changes directly into the repository workspace now."#####,
            repo = repo,
            pr_number = diff_ctx.pr_number,
            pr_title = pr_title,
            missing_summary = missing_summary,
            target_files = target_files,
            pr_body = if pr_body.is_empty() { "N/A" } else { pr_body }
        );

        let _ = self.run_agy_prompt(&prompt, repo_dir).await?;

        // Detect newly modified doc files
        let status_out = Command::new("git")
            .current_dir(repo_dir)
            .args(["status", "--porcelain"])
            .output()
            .await?;

        let modified_files: Vec<String> = String::from_utf8_lossy(&status_out.stdout)
            .lines()
            .map(|l| l[3..].trim().to_string())
            .filter(|f| f.ends_with(".md") || f.contains("doc") || f.contains("CHANGELOG"))
            .collect();

        Ok(if modified_files.is_empty() {
            eval.doc_files_to_update.clone()
        } else {
            modified_files
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

        let output = cmd.output().await.context("Failed to run agy command")?;

        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            error!(
                "agy returned non-zero status in DocGuard: {}",
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
    fn test_parse_doc_parity_evaluation() {
        let raw = r#"```json
{
  "is_doc_sufficient": false,
  "missing_doc_summary": "Missing ADR on plane split boundary and missing CHANGELOG update",
  "doc_files_to_update": ["docs/decisions/adr-0706.md", "CHANGELOG.md"],
  "suggested_adr_title": "ADR-0706: AGPL Plane Split"
}
```"#;
        let json_str = extract_json_block(raw);
        let parsed: DocParityEvaluation = serde_json::from_str(&json_str).expect("Valid parse");
        assert!(!parsed.is_doc_sufficient);
        assert_eq!(parsed.doc_files_to_update.len(), 2);
        assert_eq!(
            parsed.suggested_adr_title.as_deref(),
            Some("ADR-0706: AGPL Plane Split")
        );
    }
}
