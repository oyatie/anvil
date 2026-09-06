//! Bounding and neutralising untrusted text sent to model turns.
//!
//! Pull-request fields, diffs, review comments, file and branch names, CI logs,
//! conflict diagnostics, and model-proposed follow-up actions all originate
//! outside the harness. Each reaches a prompt as an [`Untrusted`] segment:
//! capped with the cap declared rather than applied silently, and fenced with a
//! marker the content cannot close.
//!
//! # Why a type rather than a convention
//!
//! [`Untrusted`] has a private field and exactly one way out,
//! [`Untrusted::render`], which always caps, neutralises and fences. A prompt
//! site in a sibling module therefore cannot emit contributor text undelimited
//! by forgetting to call the fence.
//!
//! A markdown code block is not that guarantee. A line of three backticks in
//! any added file closes one, and everything after it is unmarked prompt text
//! that the model reads in the harness's own voice -- which is exactly the
//! position the diff occupied while it was the one channel with no fence.

/// Maximum number of bytes of diff that may be embedded in a review prompt.
/// Beyond this the diff is capped and the cap is DECLARED.
///
/// The bound is not a style preference. A single argv argument is capped at
/// MAX_ARG_STRLEN (~128KB) on Linux, so an uncapped prompt fails the provider
/// spawn outright; the prompt travels on STDIN, but an
/// unbounded prompt still exhausts the model's context and produces a verdict
/// rendered over material the model silently dropped.
pub const MAX_DIFF_CHARS: usize = 120_000;

/// Cap on the fenced PR title.
///
/// Every attacker-controlled field is bounded, not just the diff: a 10 MB PR
/// description restores exactly the failure the diff cap was added to prevent.
/// The three field caps and `MAX_DIFF_CHARS` together keep the whole prompt
/// inside a budget the model can actually read.
pub const MAX_PR_TITLE_CHARS: usize = 2_000;

/// Cap on the fenced PR description.
pub const MAX_PR_BODY_CHARS: usize = 4_000;

/// Cap on the repository's custom rules file, which is repository-controlled
/// and therefore also attacker-controlled on a fork PR.
pub const MAX_CUSTOM_RULES_CHARS: usize = 6_000;

/// Cap on a working diff handed to an agent that edits and pushes.
///
/// Smaller than [`MAX_DIFF_CHARS`]: the reviewer reads a whole change to judge
/// it, while the fixer is told what it just broke. A bigger window there buys
/// nothing and spends the model's attention on material it will not act on.
pub const MAX_WORKING_DIFF_CHARS: usize = 60_000;

/// Cap on CI logs quoted into a prompt.
pub const MAX_CI_LOG_CHARS: usize = 20_000;

/// Cap on a document body supplied to DocGuard.
pub const MAX_DOC_BODY_CHARS: usize = 40_000;

/// Cap on the diff supplied to DocGuard. DocGuard needs less context than the
/// full reviewer and can inspect the checkout when the excerpt is incomplete.
pub const MAX_DOC_DIFF_CHARS: usize = 50_000;

/// Cap for lists of changed filenames.
pub const MAX_CHANGED_FILES_CHARS: usize = 20_000;

/// Cap for path-like contributor metadata.
pub const MAX_FILE_PATH_CHARS: usize = 4_096;

/// Cap for names (authors, branches, workflows, and document titles).
pub const MAX_NAME_CHARS: usize = 2_000;

/// Cap for merge diagnostics supplied to the queue healer.
pub const MAX_MERGE_CONFLICT_CHARS: usize = 20_000;

/// Cap for a model-proposed fix quoted into a later, write-capable turn.
pub const MAX_PROPOSED_FIX_CHARS: usize = 12_000;

// The exported `*_CHARS` names predate byte-accurate truncation and remain for
// API compatibility. Every value above is a byte limit; selection helpers move
// to UTF-8 boundaries before slicing.

pub mod label;
pub use label::UntrustedLabel;

mod selection;
pub use selection::fence_untrusted;
use selection::{
    Selection, fence_neutralised, neutralise_delimiters, neutralised_leading_excerpt,
    neutralised_len, neutralised_trailing_excerpt, truncation_notice,
};

/// One externally influenced segment of a model prompt.
///
/// The only way out of this type is [`Untrusted::render`], which always caps,
/// declares, neutralises and fences. There is no accessor handing back the text
/// as written, and the field is private to this module, so a prompt site cannot
/// emit contributor content undelimited even by mistake. Not "is checked and
/// reported"; does not compile.
pub struct Untrusted<'a> {
    label: UntrustedLabel,
    content: &'a str,
}

impl<'a> Untrusted<'a> {
    pub fn new(label: UntrustedLabel, content: &'a str) -> Self {
        Self { label, content }
    }

    /// The segment as it appears in the prompt: heading, any truncation
    /// declaration, then the fenced block.
    ///
    /// Marker counting happens over the complete source without allocating its
    /// expanded form. Only the channel-selected original head/tail excerpt is
    /// then neutralised, and that result is capped again. A marker-stuffed CI
    /// log can therefore cost time proportional to its input (measurement is
    /// necessarily a scan) but never an allocation proportional to its quoted
    /// expansion.
    ///
    /// The truncation notice sits outside the fence deliberately. Inside, it
    /// would be surrounded by text the prompt has just told the model to
    /// disregard as instructions, so the one line that has to be believed would
    /// be the one line marked as data.
    pub fn render(&self) -> String {
        let embedded_len = neutralised_len(self.content);
        let mut out = String::new();
        out.push_str(self.label.heading());
        out.push('\n');
        if embedded_len <= self.label.max_chars() {
            // The measured output is already bounded, so materialising it
            // cannot recreate the pre-cap amplification problem.
            let neutralised = neutralise_delimiters(self.content);
            out.push_str(&fence_neutralised(
                self.label,
                self.label.label(),
                &neutralised,
            ));
            out.push('\n');
            return out;
        }

        let notice = truncation_notice(
            self.content.len(),
            embedded_len,
            self.label.max_chars(),
            self.label.described(),
            self.label.selection(),
        );
        out.push_str(&notice);
        let content_budget = self.label.max_chars().saturating_sub(notice.len());
        match self.label.selection() {
            Selection::Leading => {
                let capped = neutralised_leading_excerpt(self.content, content_budget);
                out.push_str(&fence_neutralised(self.label, self.label.label(), &capped));
            }
            Selection::Trailing => {
                let capped = neutralised_trailing_excerpt(self.content, content_budget);
                out.push_str(&fence_neutralised(self.label, self.label.label(), &capped));
            }
            Selection::HeadAndTail => {
                let head_budget = content_budget.div_ceil(2);
                let tail_budget = content_budget / 2;
                let head = neutralised_leading_excerpt(self.content, head_budget);
                let tail = neutralised_trailing_excerpt(self.content, tail_budget);
                out.push_str(&fence_neutralised(self.label, "WORKING_DIFF_HEAD", &head));
                out.push('\n');
                out.push_str(&fence_neutralised(self.label, "WORKING_DIFF_TAIL", &tail));
            }
        }
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests;
