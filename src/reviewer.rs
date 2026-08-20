use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{info, warn};

use crate::ai_driver::{ModelExecutionConfig, SubscriptionExecutor};
use crate::git_manager::PrDiffContext;

#[path = "reviewer/lens_feedback_engine.rs"]
pub mod lens_feedback_engine;

pub use lens_feedback_engine::{
    CanonicalLens, LensEvaluationFinding, LensFeedbackEngine, LensFeedbackReport,
    LensFindingSeverity,
};

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
        let prompt = self.build_prompt(diff_ctx, pr_title, pr_body, &custom_rules);

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
        if let Some(path) = &self.rules_path {
            if path.exists() {
                if let Ok(content) = tokio::fs::read_to_string(path).await {
                    return content;
                }
            }
        }
        String::new()
    }

    fn build_prompt(
        &self,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
        custom_rules: &str,
    ) -> String {
        let mode_desc = if diff_ctx.is_incremental {
            format!(
                "INCREMENTAL REVIEW (Delta commits since previous review SHA {} to current HEAD {})",
                diff_ctx.previous_head_sha.as_deref().unwrap_or("unknown"),
                diff_ctx.head_sha
            )
        } else {
            format!(
                "FULL PR REVIEW (Base: {}, Head: {})",
                diff_ctx.base_branch, diff_ctx.head_sha
            )
        };

        let rules_section = if !custom_rules.is_empty() {
            format!(
                "\n### Custom Repository Engineering Rules:\n{}\n",
                custom_rules
            )
        } else {
            String::new()
        };

        let mut prompt = String::new();
        prompt.push_str("You are Anvil, the Autonomous Code Review & Adversarial Quality Sentinel for Oyatie and Console.\n");
        prompt.push_str("You evaluate Pull Requests using a 16-Lens Canonical Reasoning Framework and emit structured JSON reviews.\n\n");
        prompt.push_str(&format!("## PR Metadata:\n- **Repository**: {}\n- **PR Number**: #{}\n- **Mode**: {}\n- **PR Title**: {}\n- **PR Description**:\n{}\n{}\n",
            diff_ctx.repo, diff_ctx.pr_number, mode_desc, pr_title, pr_body, rules_section));

        prompt.push_str("## Canonical 16-Lens Adversarial Review Rubric:\n");
        prompt.push_str("1. Cartesian doubt: Question foundational assumptions. Is this change actually solving the real root problem?\n");
        prompt.push_str("2. Essentialism / YAGNI: Is this code minimal and necessary, or over-engineered with speculative abstractions?\n");
        prompt.push_str("3. Chesterton's Fence: Understand why the existing code was written before approving alterations or deletions.\n");
        prompt.push_str("4. Contrarian / Outside-the-box: Is there an unorthodox, dramatically simpler 10x architectural alternative?\n");
        prompt.push_str("5. Socratic: Challenge interfaces, boundary contracts, and invariants with clarifying inquiries.\n");
        prompt.push_str("6. Pragmatism: Balance theoretical purity against operational velocity, simplicity, and maintainability.\n");
        prompt.push_str("7. Red Team: Actively probe for security vulnerabilities, injection vectors, TOCTOU race conditions, unauthenticated endpoints.\n");
        prompt.push_str("8. Systems Thinking: Trace non-obvious cascade effects, coupling across microservices, and hidden feedback loops.\n");
        prompt.push_str("9. Operability / Day-2: Are logs structured? Are metrics emitted? How will on-call engineers debug this at 3 AM?\n");
        prompt.push_str("10. Opportunity Cost: Does this change introduce long-term maintenance burdens that outweigh its immediate benefit?\n");
        prompt.push_str("11. Blast-radius / Cell-based: Can a failure in this component propagate across cell boundaries or bring down unrelated tenants?\n");
        prompt.push_str("12. Constant-work / Anti-fragility: Does latency degrade under heavy load? Are queues and static worker pools bounded?\n");
        prompt.push_str("13. Shared-nothing / Eventual consistency: Are distributed components decoupled? Are operations idempotent?\n");
        prompt.push_str("14. FinOps / Unit-cost: Does this increase memory allocations, cloud egress, or unbudgeted compute hotpaths?\n");
        prompt.push_str("15. Telemetry-first: Are OpenTelemetry traces, spans, and metrics instrumented across critical execution paths?\n");
        prompt.push_str("16. Zero-trust / Defense-in-depth: Validate all inputs, enforce least privilege, and sanitize external data boundaries.\n\n");

        prompt.push_str("## Response Format Instructions:\n");
        prompt.push_str(
            "You MUST respond with a single valid JSON object enclosed in a ```json codeblock.\n",
        );
        prompt.push_str("Schema:\n");
        prompt.push_str("{\n  \"summary\": \"Markdown summary with 16-lens table, executive overview, and critical risks\",\n  \"verdict\": \"APPROVE | COMMENT | REQUEST_CHANGES\",\n  \"comments\": [{\"path\": \"file.ext\", \"line\": 42, \"side\": \"RIGHT\", \"body\": \"Finding description\"}]\n}\n\n");

        prompt.push_str("## Git Diff to Review:\n```diff\n");
        prompt.push_str(&diff_ctx.diff_content);
        prompt.push_str("\n```\n");

        prompt
    }

    fn parse_review_response(&self, raw_output: &str) -> Result<ReviewResponse> {
        let json_candidate = extract_json_block(raw_output);

        match serde_json::from_str::<ReviewResponse>(&json_candidate) {
            Ok(resp) => Ok(resp),
            Err(err) => {
                // Previously this returned verdict "COMMENT", which evaluator.rs
                // treats as acceptable — so an unparseable response (a refusal,
                // an error string, truncated output, an E2BIG failure) certified
                // the PR and enlisted it in the merge queue. It now reports
                // ERRORED, which blocks. The scorecard still posts, carrying the
                // raw output, so the author sees why the review failed.
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
