pub mod prompt;
pub mod rubric;
pub mod untrusted;

pub use untrusted::{
    MAX_CUSTOM_RULES_CHARS, MAX_DIFF_CHARS, MAX_PR_BODY_CHARS, MAX_PR_TITLE_CHARS, Untrusted,
    UntrustedLabel, fence_untrusted,
};

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

use crate::ai_driver::{ModelExecutionConfig, SubscriptionExecutor};
use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineReviewComment {
    pub path: String,
    pub line: u64,
    #[serde(default = "default_side")]
    pub side: String,
    pub body: String,
}

fn default_side() -> String {
    "RIGHT".to_string()
}

/// Verdict emitted when the model's response could not be parsed at all.
///
/// This is NOT a review outcome the model can choose; it is the harness
/// reporting that no review was obtained. `evaluator.rs` maps it to a blocking
/// gate (invariant I1: absent evidence is never a pass).
pub const VERDICT_ERRORED: &str = "ERRORED";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResponse {
    pub summary: String,
    /// "APPROVE" | "COMMENT" | "REQUEST_CHANGES", or `VERDICT_ERRORED` when the
    /// harness could not parse a response.
    ///
    /// Deliberately NOT `#[serde(default)]`: a response omitting `verdict` is a
    /// parse failure, not an implicit pass. That default was one hop in the
    /// chain that let a garbage model response merge a PR.
    pub verdict: String,
    #[serde(default)]
    pub comments: Vec<InlineReviewComment>,
}

pub struct Reviewer {
    model_config: ModelExecutionConfig,
    rules_path: Option<PathBuf>,
    executor: SubscriptionExecutor,
}

impl Reviewer {
    pub fn new(model_config: ModelExecutionConfig, rules_path: Option<PathBuf>) -> Self {
        Self {
            model_config,
            rules_path,
            executor: SubscriptionExecutor::new(),
        }
    }

    pub async fn review_pr(
        &self,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
    ) -> Result<ReviewResponse> {
        let custom_rules = self.load_custom_rules().await;
        let prompt = self.build_prompt(diff_ctx, pr_title, pr_body, &custom_rules)?;

        info!(
            "Running Canonical 16-Lens Adversarial Code Review via {} for PR #{} on {} (diff length: {} chars)",
            self.model_config.provider.display_name(),
            diff_ctx.pr_number,
            diff_ctx.repo,
            diff_ctx.diff_content.len()
        );

        let output_text = self
            .executor
            .execute_prompt(&prompt, &diff_ctx.repo_working_dir, &self.model_config)
            .await?;

        self.parse_review_response(&output_text)
    }

    async fn load_custom_rules(&self) -> String {
        if let Some(path) = &self.rules_path
            && path.exists()
            && let Ok(content) = tokio::fs::read_to_string(path).await
        {
            return content;
        }
        String::new()
    }

    /// The model's answer, or a blocking verdict saying there wasn't one.
    ///
    /// An unparseable response -- a refusal, an error string, truncated output,
    /// an E2BIG failure -- reports [`VERDICT_ERRORED`], which `evaluator.rs`
    /// blocks on. Any verdict `evaluator.rs` accepts would certify the PR and
    /// enlist it in the merge queue on no review at all (invariant I1). The
    /// scorecard still posts, carrying the raw output, so the author sees why
    /// the review failed.
    fn parse_review_response(&self, raw_output: &str) -> Result<ReviewResponse> {
        let json_candidate = extract_json_block(raw_output);

        match serde_json::from_str::<ReviewResponse>(&json_candidate) {
            Ok(resp) => Ok(resp),
            Err(err) => {
                warn!(
                    "Could not parse ReviewResponse JSON: {}. Reporting verdict {} (blocking).",
                    err, VERDICT_ERRORED
                );
                Ok(ReviewResponse {
                    summary: format!(
                        "AI review could not be parsed and did not produce a verdict.\n\nParse error: {}\n\nRaw model output:\n{}",
                        err, raw_output
                    ),
                    verdict: VERDICT_ERRORED.to_string(),
                    comments: Vec::new(),
                })
            }
        }
    }
}

fn extract_json_block(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_block_markdown() {
        let text = "Here is the review:\n```json\n{\"summary\":\"Good\",\"verdict\":\"APPROVE\",\"comments\":[]}\n```\nThanks!";
        let extracted = extract_json_block(text);
        assert_eq!(
            extracted,
            "{\"summary\":\"Good\",\"verdict\":\"APPROVE\",\"comments\":[]}"
        );
    }

    #[test]
    fn test_extract_json_block_raw() {
        let text = "{\"summary\":\"Good\",\"verdict\":\"APPROVE\",\"comments\":[]}";
        let extracted = extract_json_block(text);
        assert_eq!(extracted, text);
    }

    fn reviewer() -> Reviewer {
        Reviewer::new(ModelExecutionConfig::default(), None)
    }

    /// The headline regression: an unparseable model response must NOT yield a
    /// verdict that certifies. Before this fix every one of these returned
    /// "COMMENT", which evaluator.rs accepted, which enlisted the PR.
    #[test]
    fn unparseable_responses_report_errored_not_comment() {
        let garbage = [
            "I cannot help with that request.",
            "",
            "error: agy: command not found",
            "Usage: claude [OPTIONS] --print <PROMPT>",
            "{\"summary\": \"truncated...",
        ];
        for raw in garbage {
            let resp = reviewer()
                .parse_review_response(raw)
                .expect("parse must not error out");
            assert_eq!(
                resp.verdict, VERDICT_ERRORED,
                "unparseable input {:?} must report ERRORED, got {:?}",
                raw, resp.verdict
            );
            assert_ne!(resp.verdict, "COMMENT", "input {:?} must not pass", raw);
        }
    }

    /// A response omitting `verdict` is a parse failure, not an implicit pass.
    /// This was the second hop in the same chain.
    #[test]
    fn missing_verdict_field_is_not_an_implicit_pass() {
        let resp = reviewer()
            .parse_review_response("{\"summary\":\"looks fine\",\"comments\":[]}")
            .expect("parse must not error out");
        assert_eq!(resp.verdict, VERDICT_ERRORED);
    }

    #[test]
    fn well_formed_responses_still_parse_normally() {
        for (raw, want) in [
            (
                "{\"summary\":\"ok\",\"verdict\":\"APPROVE\",\"comments\":[]}",
                "APPROVE",
            ),
            (
                "{\"summary\":\"ok\",\"verdict\":\"COMMENT\",\"comments\":[]}",
                "COMMENT",
            ),
            (
                "{\"summary\":\"no\",\"verdict\":\"REQUEST_CHANGES\",\"comments\":[]}",
                "REQUEST_CHANGES",
            ),
        ] {
            let resp = reviewer().parse_review_response(raw).expect("parses");
            assert_eq!(resp.verdict, want);
        }
    }

    /// The errored summary must carry the raw output so the scorecard explains
    /// the failure rather than silently showing nothing.
    #[test]
    fn errored_summary_preserves_raw_output_for_the_author() {
        let resp = reviewer()
            .parse_review_response("agy exited with status 127")
            .expect("parses");
        assert!(resp.summary.contains("agy exited with status 127"));
        assert!(resp.comments.is_empty());
    }
}
