//! Assembling the review prompt out of pieces that know who wrote them.
//!
//! Every string in a review prompt has one of two authors: this harness, or the
//! contributor whose pull request is under review. [`Part`] makes that a type
//! distinction rather than a comment, and [`assemble`] is the single place a
//! prompt is built, so "no contributor segment reaches the model unfenced" is a
//! property of the assembly instead of a convention each site must remember.

use super::Reviewer;
use super::rubric;
use super::untrusted::{Untrusted, UntrustedLabel};
use crate::git_manager::PrDiffContext;

/// One piece of a review prompt, tagged by its author.
///
/// The contributor variant can only hold an [`Untrusted`], whose sole output is
/// fenced and capped. There is no variant that carries contributor text as a
/// bare `String`, so [`assemble`] has no branch that could emit one raw.
enum Part<'a> {
    Harness(String),
    Contributor(Untrusted<'a>),
}

/// Joins the parts in order, rendering every contributor part through its fence.
fn assemble(parts: &[Part<'_>]) -> String {
    let mut out = String::new();
    for part in parts {
        match part {
            Part::Harness(text) => out.push_str(text),
            Part::Contributor(untrusted) => out.push_str(&untrusted.render()),
        }
        out.push('\n');
    }
    out
}

/// Who the model is and what it is being asked for.
const PREAMBLE: &str = "You are Anvil, the Autonomous Code Review & Adversarial Quality Sentinel for Oyatie and Console.\n\
     You evaluate Pull Requests using a 16-Lens Canonical Reasoning Framework and emit structured JSON reviews.\n";

/// The schema the harness parses back. Emitted last on purpose; see
/// [`Reviewer::build_prompt`].
const RESPONSE_FORMAT: &str = "## Response Format Instructions:\n\
     You MUST respond with a single valid JSON object enclosed in a ```json codeblock.\n\
     Schema:\n\
     {\n  \"summary\": \"Markdown summary with 16-lens table, executive overview, and critical risks\",\n  \"verdict\": \"APPROVE | COMMENT | REQUEST_CHANGES\",\n  \"comments\": [{\"path\": \"file.ext\", \"line\": 42, \"side\": \"RIGHT\", \"body\": \"Finding description\"}]\n}\n";

/// Repository, PR number and what range the diff covers. Harness-authored: the
/// repository name and the SHAs come from the forge, not from the PR body.
fn metadata_block(diff_ctx: &PrDiffContext) -> String {
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
    format!(
        "## PR Metadata:\n- **Repository**: {}\n- **PR Number**: #{}\n- **Mode**: {mode_desc}\n",
        diff_ctx.repo, diff_ctx.pr_number
    )
}

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
    ) -> String {
        let mut parts = vec![
            Part::Harness(PREAMBLE.to_string()),
            Part::Harness(metadata_block(diff_ctx)),
            Part::Contributor(Untrusted::new(UntrustedLabel::PrTitle, pr_title)),
            Part::Contributor(Untrusted::new(UntrustedLabel::PrDescription, pr_body)),
        ];
        if !custom_rules.is_empty() {
            let rules = Untrusted::new(UntrustedLabel::CustomRules, custom_rules);
            parts.push(Part::Contributor(rules));
        }
        parts.push(Part::Harness(rubric::rubric_prompt()));
        let diff = Untrusted::new(UntrustedLabel::GitDiff, &diff_ctx.diff_content);
        parts.push(Part::Contributor(diff));
        parts.push(Part::Harness(RESPONSE_FORMAT.to_string()));
        assemble(&parts)
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
            base_sha: "base".to_string(),
            head_sha: "head".to_string(),
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
        let prompt = reviewer().build_prompt(
            &ctx(&"d".repeat(MAX_DIFF_CHARS * 4)),
            &"t".repeat(MAX_PR_TITLE_CHARS * 4),
            &"b".repeat(MAX_PR_BODY_CHARS * 4),
            &"r".repeat(MAX_CUSTOM_RULES_CHARS * 4),
        );

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
        // And every cut is declared: a bounded prompt that hides what it
        // dropped just moves the fabrication from the size to the verdict.
        assert_eq!(
            prompt.matches("[TRUNCATED:").count(),
            4,
            "all four oversized fields must declare their truncation"
        );
    }

    /// The marker-stuffed case the cap ordering exists for: quoting the fence
    /// marker grows the text ~3.7x, so a cap applied before quoting would let a
    /// 120 KB diff land at 440 KB.
    #[test]
    fn a_marker_stuffed_diff_is_still_bounded_and_still_fenced() {
        let prompt = reviewer().build_prompt(
            &ctx(&"END UNTRUSTED GIT_DIFF\n".repeat(20_000)),
            "t",
            "b",
            "",
        );
        let ceiling = MAX_DIFF_CHARS + 20_000;
        assert!(
            prompt.len() <= ceiling,
            "a diff of nothing but the fence marker inflated the prompt to {} \
             bytes, over the {ceiling}-byte ceiling: the cap runs before the \
             quoting that grows the text",
            prompt.len()
        );
        assert_eq!(
            prompt.matches("END UNTRUSTED GIT_DIFF").count(),
            1,
            "the closing delimiter survives {} times: a suffix cut of the \
             quoted text spliced the marker back onto its label",
            prompt.matches("END UNTRUSTED GIT_DIFF").count()
        );
    }

    /// The harness gets the last word. The diff is the largest thing an author
    /// controls, and end-of-context is the strongest position in a prompt.
    #[test]
    fn the_response_format_instructions_follow_the_diff() {
        let prompt = reviewer().build_prompt(&ctx("+let x = 1;\n"), "t", "b", "");
        let diff_at = prompt.find("END UNTRUSTED GIT_DIFF").expect("diff fenced");
        let schema_at = prompt
            .find("## Response Format Instructions:")
            .expect("schema present");
        assert!(
            diff_at < schema_at,
            "the diff ends at {diff_at} but the response format is at \
             {schema_at}: contributor content has the last word in the context"
        );
    }
}
