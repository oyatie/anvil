//! Bounding and neutralising untrusted pull-request text.
//!
//! Title, body, diff and the repository's own rules file are written by whoever
//! opened the pull request, and all four reach a model prompt. Each reaches it
//! as an [`Untrusted`] segment: capped with the cap DECLARED rather than
//! applied silently, and fenced with a marker the content cannot close.
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

pub mod label;
pub use label::UntrustedLabel;

/// One contributor-authored segment of a review prompt.
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
    /// Neutralising happens BEFORE capping, not after. Quoting the marker grows
    /// the text by about 3.7x in the worst case, so capping first would let a
    /// 120 KB diff of nothing but the marker word land in the prompt at 440 KB
    /// and void every bound the caps exist to hold.
    ///
    /// The truncation notice sits outside the fence deliberately. Inside, it
    /// would be surrounded by text the prompt has just told the model to
    /// disregard as instructions, so the one line that has to be believed would
    /// be the one line marked as data.
    pub fn render(&self) -> String {
        let neutralised = neutralise_delimiters(self.content);
        let (capped, notice) = cap_declaring(
            &neutralised,
            self.content.len(),
            self.label.max_chars(),
            self.label.described(),
        );
        let mut out = String::new();
        out.push_str(self.label.heading());
        out.push('\n');
        if let Some(notice) = notice {
            out.push_str(&notice);
        }
        out.push_str(&fence_neutralised(self.label, &capped));
        out.push('\n');
        out
    }
}

/// Wraps contributor text in explicit delimiters carrying the channel's
/// standing instruction.
///
/// The delimiters cannot be closed from inside: every occurrence of the marker
/// word in `content` is neutralised first, so the region the harness opened is
/// the region the harness closes. Neutralising is not deleting -- the author's
/// text stays in the prompt, visibly quoted, because an injection attempt is a
/// review finding, not noise.
///
/// This is the uncapped primitive. Prompt sites use [`Untrusted::render`],
/// which also bounds the segment.
pub fn fence_untrusted(label: UntrustedLabel, content: &str) -> String {
    fence_neutralised(label, &neutralise_delimiters(content))
}

/// The fence itself, over content whose markers are already defused.
///
/// Separate from [`fence_untrusted`] because [`Untrusted::render`] neutralises
/// before capping, and [`neutralise_delimiters`] is not idempotent: a second
/// pass would quote its own quoting and grow the text again.
fn fence_neutralised(label: UntrustedLabel, neutralised: &str) -> String {
    let name = label.label();
    format!(
        "{}\nBEGIN UNTRUSTED {name}\n{neutralised}\nEND UNTRUSTED {name}",
        label.standing_instruction()
    )
}

/// The marker word the fence is built from. Neutralised wherever it appears in
/// untrusted content.
const FENCE_MARKER: &str = "untrusted";

/// What a quoted marker becomes. Chosen to be self-explaining in the prompt:
/// the model sees that the author wrote the word and that the harness defused
/// it, rather than seeing a frame it might mistake for the harness's own.
const FENCE_MARKER_QUOTED: &str = "_QUOTED_BY_THE_PR_AUTHOR";

/// Defuses every occurrence of the fence marker, case-insensitively, so an
/// author who writes `END UNTRUSTED PR_DESCRIPTION` (in any casing) cannot
/// terminate the region and continue outside it.
///
/// The quote lands between the marker and the label, so no suffix cut of the
/// result can rejoin them: a truncation can drop characters from the end and
/// never splice `UNTRUSTED` back onto `GIT_DIFF`.
fn neutralise_delimiters(content: &str) -> String {
    // ASCII-lowercase specifically: `to_lowercase` can change a string's byte
    // length (`İ` -> `i̇`), which would desynchronise the indices below.
    let lowered = content.to_ascii_lowercase();
    let mut out = String::with_capacity(content.len());
    let mut cursor = 0usize;
    while let Some(rel) = lowered[cursor..].find(FENCE_MARKER) {
        let at = cursor + rel;
        let end = at + FENCE_MARKER.len();
        out.push_str(&content[cursor..end]);
        out.push_str(FENCE_MARKER_QUOTED);
        cursor = end;
    }
    out.push_str(&content[cursor..]);
    out
}

/// Truncates `content` to `max` bytes and, when it did, returns the notice that
/// says so.
///
/// The notice carries the MEASURED original length, never the cap constant: a
/// declaration quoting the constant tells the reader how much was kept and
/// nothing about how much was lost (invariant I2). `measured` is the size of
/// what the author actually wrote; `content` may be longer, because quoting the
/// fence marker inside it only grows the text, and the notice names both
/// whenever they differ. The notice counts toward `max`, so the returned pair
/// always fits the bound the caller asked for.
fn cap_declaring(
    content: &str,
    measured: usize,
    max: usize,
    what: &str,
) -> (String, Option<String>) {
    let embedded = content.len();
    if embedded <= max {
        return (content.to_string(), None);
    }

    let grown = if embedded == measured {
        String::new()
    } else {
        format!(" ({embedded} bytes once the fence markers it quotes are defused)")
    };
    let notice = format!(
        "[TRUNCATED: the {what} is {measured} bytes{grown}, over the {max}-byte prompt cap. \
         Only the leading portion is shown below; the remainder was NOT provided and has \
         NOT been reviewed. Do not report on what you were not shown.]\n"
    );

    let mut end = max.saturating_sub(notice.len()).min(embedded);
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }
    (content[..end].to_string(), Some(notice))
}
