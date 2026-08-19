use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::warn;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFeedbackItem {
    pub comment_id: Option<u64>,
    pub file_path: Option<String>,
    pub line: Option<u64>,
    pub body: String,
    pub author: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemEvaluation {
    pub item_index: usize,
    pub is_valid: bool,
    pub rationale: String,
    #[serde(default)]
    pub files_to_edit: Vec<String>,
    #[serde(default)]
    pub proposed_fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub evaluations: Vec<ItemEvaluation>,
}

pub async fn evaluate_feedback_items(
    repo: &str,
    repo_dir: &Path,
    feedback_items: &[ReviewFeedbackItem],
    agy_effort: &str,
) -> Result<EvaluationResult> {
    let mut prompt = format!(
        "You are Oyatie's Senior Principal Engineer. Evaluate the following code review feedback items for repository `{}` to determine if each item is a **Valid Issue** or a **False Signal**.\n\n",
        repo
    );

    prompt.push_str("## Review Feedback Items:\n");
    for (i, item) in feedback_items.iter().enumerate() {
        prompt.push_str(&format!(
            "### Item [{}]\n- **Author**: {}\n- **File**: {}\n- **Line**: {}\n- **Comment**:\n{}\n\n",
            i,
            item.author,
            item.file_path.as_deref().unwrap_or("General PR"),
            item.line.map(|l| l.to_string()).unwrap_or_else(|| "N/A".to_string()),
            item.body
        ));
    }

    prompt.push_str(r#####"## Evaluation Instructions:
1. Cross-reference each comment with the actual codebase in this workspace.
2. Determine:
   - `is_valid`: `true` if this is a legitimate bug, missing type validation, concurrency issue, security risk, or performance regression requiring code changes.
   - `is_valid`: `false` if this is a false positive, misunderstood intent, already handled by another layer, or invalid suggestion.
3. Provide a clear technical `rationale` for each decision.

## Output Format:
Return strictly valid JSON matching this schema:
```json
{
  "evaluations": [
    {
      "item_index": 0,
      "is_valid": true,
      "rationale": "Clear technical explanation of why valid or why false signal",
      "files_to_edit": ["path/to/file.ext"],
      "proposed_fix": "Description of exact change needed"
    }
  ]
}
```
"#####);

    let output = run_agy(agy_effort, &prompt, repo_dir).await?;
    let json_candidate = extract_json_block(&output);

    match serde_json::from_str::<EvaluationResult>(&json_candidate) {
        Ok(res) => Ok(res),
        Err(e) => {
            warn!("Failed to parse evaluation JSON: {}. Defaulting all items to valid.", e);
            let default_evals = feedback_items
                .iter()
                .enumerate()
                .map(|(i, it)| ItemEvaluation {
                    item_index: i,
                    is_valid: true,
                    rationale: format!("Addressed feedback: {}", it.body),
                    files_to_edit: it.file_path.clone().into_iter().collect(),
                    proposed_fix: None,
                })
                .collect();
            Ok(EvaluationResult {
                evaluations: default_evals,
            })
        }
    }
}

pub fn extract_json_block(text: &str) -> String {
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

async fn run_agy(effort: &str, prompt: &str, working_dir: &Path) -> Result<String> {
    let mut cmd = Command::new("agy");
    cmd.args([
        "--print",
        prompt,
        "--effort",
        effort,
        "--dangerously-skip-permissions",
    ]);
    cmd.current_dir(working_dir);
    let output = cmd.output().await.context("Failed to run agy")?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_evaluation_result() {
        let raw = r#"```json
{
  "evaluations": [
    {
      "item_index": 0,
      "is_valid": false,
      "rationale": "The function explicitly checks for nil at line 12 before returning.",
      "files_to_edit": [],
      "proposed_fix": null
    },
    {
      "item_index": 1,
      "is_valid": true,
      "rationale": "Missing error propagation in async handler.",
      "files_to_edit": ["src/server.rs"],
      "proposed_fix": "Add ? operator to db call"
    }
  ]
}
```"#;
        let json_str = extract_json_block(raw);
        let parsed: EvaluationResult = serde_json::from_str(&json_str).expect("Valid parse");
        assert_eq!(parsed.evaluations.len(), 2);
        assert!(!parsed.evaluations[0].is_valid);
        assert!(parsed.evaluations[1].is_valid);
        assert_eq!(parsed.evaluations[1].files_to_edit, vec!["src/server.rs"]);
    }
}
