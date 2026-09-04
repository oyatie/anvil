//! Assembling a typed review prompt from pieces with known authorship.

use super::Reviewer;
use super::rubric;
use super::untrusted::{Untrusted, UntrustedLabel};
use crate::git_manager::PrDiffContext;
use crate::model_prompt::{HarnessText, ModelPrompt};
use anyhow::Result;

impl Reviewer {
    /// The prompt one review runs on.
    ///
    /// Every argument that a pull request author controls -- title, body, the
    /// checked-out rules file, and the diff -- is wrapped in [`Untrusted`] here
    /// and nowhere else, so each is fenced, capped and declared by construction.
    ///
    /// Ordering is load-bearing at one point. The diff is the contributor's
    /// largest channel, and it is emitted BEFORE the response-format
    /// instructions so that the last thing in the model's context is written by
    /// the harness rather than by the author of the code under review.
    pub fn build_prompt(
        &self,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
        custom_rules: &str,
    ) -> Result<ModelPrompt> {
        let mut prompt = ModelPrompt::builder();
        prompt.push_harness(HarnessText::ReviewerPreambleAndRepository);
        prompt.push_repository(&diff_ctx.repo)?;
        prompt
            .push_harness(HarnessText::ReviewerPrNumber)
            .push_u64(diff_ctx.pr_number)
            .push_harness(HarnessText::ReviewerMode);
        if diff_ctx.is_incremental {
            prompt.push_harness(HarnessText::ReviewerIncrementalMode);
            if let Some(previous) = diff_ctx.previous_head_sha.as_deref() {
                prompt.push_commit_sha(previous)?;
            } else {
                prompt.push_harness(HarnessText::ReviewerUnknownPreviousSha);
            }
            prompt.push_harness(HarnessText::ReviewerToCurrentHead);
            prompt.push_commit_sha(&diff_ctx.head_sha)?;
        } else {
            prompt.push_harness(HarnessText::ReviewerFullMode);
            prompt.push_commit_sha(&diff_ctx.head_sha)?;
        }
        prompt
            .push_harness(HarnessText::ReviewerBaseBranch)
            .push_untrusted(Untrusted::new(
                UntrustedLabel::BranchName,
                &diff_ctx.base_branch,
            ))
            .push_untrusted(Untrusted::new(UntrustedLabel::PrTitle, pr_title))
            .push_untrusted(Untrusted::new(UntrustedLabel::PrDescription, pr_body));
        if !custom_rules.is_empty() {
            prompt.push_untrusted(Untrusted::new(UntrustedLabel::CustomRules, custom_rules));
        }
        rubric::append_to(&mut prompt);
        prompt
            .push_untrusted(Untrusted::new(
                UntrustedLabel::GitDiff,
                &diff_ctx.diff_content,
            ))
            .push_harness(HarnessText::ReviewerResponseFormat);
        prompt.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_driver::ModelExecutionConfig;
    use crate::git_manager::{SubjectRoot, Uncloned};
    use crate::reviewer::{
        MAX_CUSTOM_RULES_CHARS, MAX_DIFF_CHARS, MAX_PR_BODY_CHARS, MAX_PR_TITLE_CHARS,
    };
    use std::path::PathBuf;

    fn ctx(diff: &str) -> PrDiffContext {
        PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 1,
            base_branch: "main".to_string(),
            base_sha: "ba5eba5e".to_string(),
            head_sha: "deadbeef".to_string(),
            previous_head_sha: None,
            repo_working_dir: SubjectRoot::asserted(PathBuf::from("."), Uncloned::TestFixture),
            diff_content: diff.to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            is_incremental: false,
        }
    }

    fn reviewer() -> Reviewer {
        Reviewer::new(ModelExecutionConfig::default(), None)
    }

    /// Each field cap is checked on its own by the lane tests; none of them
    /// pushes every field over at once. This does, because the caps only buy
    /// an absolute ceiling if they hold together: a prompt that no pull
    /// request can inflate is the whole point of capping any of them.
    #[test]
    fn the_prompt_is_bounded_with_every_untrusted_field_oversized() {
        let prompt = reviewer()
            .build_prompt(
                &ctx(&"d".repeat(MAX_DIFF_CHARS * 4)),
                &"t".repeat(MAX_PR_TITLE_CHARS * 4),
                &"b".repeat(MAX_PR_BODY_CHARS * 4),
                &"r".repeat(MAX_CUSTOM_RULES_CHARS * 4),
            )
            .expect("valid metadata");

        // The four caps plus the fixed preamble, rubric, schema and fences.
        let ceiling = MAX_DIFF_CHARS
            + MAX_PR_TITLE_CHARS
            + MAX_PR_BODY_CHARS
            + MAX_CUSTOM_RULES_CHARS
            + 8_000;
        assert!(
            prompt.len() <= ceiling,
            "prompt is {} bytes, over the {ceiling}-byte ceiling",
            prompt.len()
        );
    }
}
