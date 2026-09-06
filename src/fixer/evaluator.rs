use crate::model_prompt::{HarnessText, ModelPrompt};
use crate::reviewer::untrusted::{Untrusted, UntrustedLabel};
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;

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

/// Builds the evaluator prompt from explicitly classified review fields.
pub fn build_feedback_evaluation_prompt(
    repo: &str,
    feedback_items: &[ReviewFeedbackItem],
) -> Result<ModelPrompt> {
    let mut prompt = ModelPrompt::builder();
    prompt.push_harness(HarnessText::EvaluatorPreambleAndRepository);
    prompt.push_repository(repo)?;
    prompt.push_harness(HarnessText::EvaluatorRepositoryEnd);
    for (i, item) in feedback_items.iter().enumerate() {
        prompt
            .push_harness(HarnessText::EvaluatorItemStart)
            .push_usize(i)
            .push_harness(HarnessText::EvaluatorItemEnd);
        if let Some(path) = item.file_path.as_deref() {
            prompt.push_untrusted(Untrusted::new(UntrustedLabel::FilePath, path));
        } else {
            prompt.push_harness(HarnessText::EvaluatorGeneralPath);
        }
        prompt.push_harness(HarnessText::EvaluatorLine);
        if let Some(line) = item.line {
            prompt.push_u64(line);
        } else {
            prompt.push_harness(HarnessText::EvaluatorNotApplicable);
        }
        prompt
            .push_harness(HarnessText::EvaluatorFieldEnd)
            .push_untrusted(Untrusted::new(UntrustedLabel::ReviewAuthor, &item.author))
            .push_untrusted(Untrusted::new(UntrustedLabel::ReviewComment, &item.body))
            .push_harness(HarnessText::EvaluatorItemBoundary);
    }

    prompt.push_harness(HarnessText::EvaluatorResponseContract);
    prompt.finish()
}

pub async fn evaluate_feedback_items(
    repo: &str,
    repo_dir: &Path,
    feedback_items: &[ReviewFeedbackItem],
    agy_effort: &str,
) -> Result<EvaluationResult> {
    let prompt = build_feedback_evaluation_prompt(repo, feedback_items)?;

    let output = run_agy(agy_effort, &prompt, repo_dir).await?;
    let json_candidate = extract_json_block(&output);

    evaluation_result(&json_candidate, feedback_items.len()).map_err(|reason| {
        anyhow::anyhow!(
            "invalid evaluation JSON: {reason}; no review feedback was authorized for editing"
        )
    })
}

fn evaluation_result(json: &str, expected: usize) -> std::result::Result<EvaluationResult, String> {
    serde_json::from_str::<EvaluationResult>(json)
        .map_err(|error| error.to_string())
        .and_then(|result| validate_total_evaluation(result, expected))
}

fn validate_total_evaluation(
    mut result: EvaluationResult,
    expected: usize,
) -> std::result::Result<EvaluationResult, String> {
    let mut seen = std::collections::BTreeSet::new();
    for evaluation in &result.evaluations {
        if evaluation.item_index >= expected {
            return Err(format!(
                "evaluation index {} is outside 0..{expected}",
                evaluation.item_index
            ));
        }
        if !seen.insert(evaluation.item_index) {
            return Err(format!(
                "evaluation index {} occurs more than once",
                evaluation.item_index
            ));
        }
    }
    if seen.len() != expected {
        let missing = (0..expected)
            .filter(|index| !seen.contains(index))
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("evaluation omitted item indices: {missing}"));
    }
    result
        .evaluations
        .sort_by_key(|evaluation| evaluation.item_index);
    Ok(result)
}

pub fn extract_json_block(text: &str) -> String {
    let json_block_re = Regex::new(r"(?s)```(?:json)?\s*(\{.*?\})\s*```").unwrap();
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

async fn run_agy(effort: &str, prompt: &ModelPrompt, working_dir: &Path) -> Result<String> {
    let budget = crate::exec::ExecClass::Model.timeout();
    let cmd = crate::exec::agy_agent(
        &crate::exec::Posture::in_workspace(working_dir),
        effort,
        budget,
        None,
    )?;
    let turn = crate::exec::turn::run(cmd, prompt, budget, "agy evaluation")
        .await
        .context("Failed to run agy")?;
    // `into_result` and not `turn.response`: preserve a failed or timed-out
    // turn as an error rather than losing its status and trying to parse its
    // empty response. Historically that parse failure fabricated a valid
    // verdict for every review comment; invalid evaluations are now rejected.
    let response = turn.into_result()?;
    // An empty answer from a turn that exited zero is still no judgment. Reject
    // it explicitly so absent evidence cannot be mistaken for a measurement
    // or authorize edits (I1).
    if response.trim().is_empty() {
        anyhow::bail!(
            "agy evaluation returned no output, so nothing judged these review \
             comments; defaulting them to valid would fabricate the verdict"
        );
    }
    Ok(response)
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

    #[test]
    fn malformed_or_incomplete_model_evaluations_authorize_nothing() {
        for json in [
            "not json",
            r#"{"evaluations":[]}"#,
            r#"{"evaluations":[{"item_index":0,"is_valid":false,"rationale":"only one"}]}"#,
            r#"{"evaluations":[{"item_index":0,"is_valid":false,"rationale":"first"},{"item_index":0,"is_valid":false,"rationale":"duplicate"}]}"#,
            r#"{"evaluations":[{"item_index":0,"is_valid":false,"rationale":"first"},{"item_index":9,"is_valid":false,"rationale":"extra"}]}"#,
        ] {
            evaluation_result(json, 2)
                .expect_err("invalid model output must not fabricate edit authorization");
        }
    }

    #[test]
    fn total_evaluation_is_sorted_and_preserved() {
        let json = r#"{"evaluations":[{"item_index":1,"is_valid":false,"rationale":"second"},{"item_index":0,"is_valid":true,"rationale":"first"}]}"#;
        let result = evaluation_result(json, 2).expect("total response must remain accepted");
        assert_eq!(result.evaluations[0].item_index, 0);
        assert_eq!(result.evaluations[1].item_index, 1);
        assert!(result.evaluations[0].is_valid);
        assert!(!result.evaluations[1].is_valid);
    }
}
