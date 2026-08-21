//! The Product seat's measurement: the bet and the acceptance bar.
//!
//! ADR-0002, Discover §1. Job: the bet and the acceptance bar. Artifact: a
//! written problem plus a done-when. Measurement: quality sign-off cannot sign
//! off without it.
//!
//! The artifact is authored on the change under review, so this gate reads the
//! change's own body — the same metadata `doc_guard` and `reviewer` already
//! receive — and makes no network or filesystem call. That last claim is not
//! decoration: `the_verdict_depends_on_nothing_but_the_change_it_was_handed`
//! pins it, because a gate that loaded its vocabulary from disk would be a
//! flake this suite could not attribute.
//!
//! The body and nothing else. An earlier revision of these signatures also took
//! the pull request title, and no test in the suite could tell a gate that read
//! it from one that ignored it — a named input with no measurement, inside a
//! gate whose whole subject is named gates with no measurement. The suite does
//! not pin this and says so: with no title parameter there is nothing for a
//! behavioural test to falsify, so it is a decision recorded in these module
//! docs rather than a test. Listed in open_questions as a decision a human can
//! veto.
//!
//! Absence is the defect itself. A change that never wrote a bar has not
//! produced evidence this gate could not read; it produced no bar. That is
//! `Failed`, not `NotMeasured` and not `Warning` — and that holds for a body
//! that uses no heading this gate recognises just as much as for an empty one.
//!
//! Two entry points, deliberately: `missing_artifacts` is the measurement and
//! `judge` is the verdict rendered from it. The split exists so the tests can
//! assert *which* artifact the gate found missing without pattern-matching on
//! the prose of the message.
//!
//! # The rules this module implements
//!
//! The specification is `tests/product_seat_done_when_test.rs`, which was
//! written and reviewed to approval before a line of this file existed. What
//! follows is the shape of the answer, not a second copy of the specification.
//!
//! * **A marker is a heading line, never a phrase.** An ATX heading of any
//!   depth, or a bold-only label, whose text is "Problem" or "Done when" in any
//!   case, with or without a trailing colon and with any whitespace around it.
//!   A colon may carry the section's whole content on the marker's own line.
//! * **A section runs to the next boundary.** A marker always ends the section
//!   above it, whatever depth it sits at. A bold-only label ends it too. An ATX
//!   heading at the marker's own depth or shallower is a sibling and ends it; a
//!   heading DEEPER than the marker is a sub-heading, and what sits under it is
//!   still inside the section. A colon-terminated line is ordinary writing and
//!   never a boundary. Those last two are decisions the suite makes on this
//!   module's behalf, each with a stated cost and a veto.
//! * **The artifact is CONTENT, not a heading.** A section carries the artifact
//!   when any line in it survives: strip the invisible characters, the bullet,
//!   the checkbox and the template comment; drop a colon-terminated lead-in and
//!   a line that is nothing but punctuation; drop a deferral (a placeholder
//!   token alone, or one announcing itself with a colon or a dash) and a line
//!   whose whole content is a pointer somewhere else. Whatever is left is the
//!   author's own writing, however terse.
//!
//! Both halves of every one of those rules are pinned in both directions,
//! because a gate that rejects a real acceptance bar is the same defect as one
//! that accepts a pasted template, pointed the other way.

use std::borrow::Cow;

use super::GateStatus;

/// One half of the Product seat's artifact.
///
/// Ordered so a caller can compare sets without depending on render order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Artifact {
    /// The bet: what is wrong and why it matters.
    WrittenProblem,
    /// The acceptance bar: how anyone other than the author checks it is done.
    DoneWhenBar,
}

impl Artifact {
    /// What the author has to write, named so they can act on it without
    /// reading this source.
    fn describe(self) -> &'static str {
        match self {
            Artifact::WrittenProblem => {
                "written problem statement (what is wrong, and why it matters)"
            }
            Artifact::DoneWhenBar => {
                "done-when acceptance bar (how someone other than the author checks \
                 this change is finished)"
            }
        }
    }
}

/// The Product artifacts this change did not produce, in canonical order.
///
/// Empty means the change carries both. This is the measurement; `judge`
/// renders its verdict and its message from it.
pub fn missing_artifacts(pr_body: &str) -> Vec<Artifact> {
    let body = without_html_comments(pr_body);

    let mut problem_written = false;
    let mut bar_written = false;
    // The section the walk is currently inside, as (which artifact, the depth
    // of the heading that opened it).
    let mut open: Option<(Artifact, usize)> = None;

    for raw in body.split('\n') {
        let line = raw.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(heading) = heading_of(trimmed) {
            match heading.marker {
                Some(artifact) => {
                    open = Some((artifact, heading.depth));
                    if is_content(heading.inline) {
                        record(artifact, &mut problem_written, &mut bar_written);
                    }
                }
                None => {
                    if let Some((_, depth)) = open
                        && (heading.bold || heading.depth <= depth)
                    {
                        open = None;
                    }
                }
            }
            continue;
        }

        if let Some((artifact, _)) = open
            && !already(artifact, problem_written, bar_written)
            && is_content(line)
        {
            record(artifact, &mut problem_written, &mut bar_written);
        }
    }

    let mut missing = Vec::new();
    if !problem_written {
        missing.push(Artifact::WrittenProblem);
    }
    if !bar_written {
        missing.push(Artifact::DoneWhenBar);
    }
    missing
}

/// Judges the Product artifact carried by the change under review.
pub fn judge(pr_body: &str) -> GateStatus {
    let missing = missing_artifacts(pr_body);
    match missing.as_slice() {
        [] => GateStatus::Passed,
        [only] => GateStatus::Failed(format!(
            "The Product artifact on this change is incomplete: it states no {}. Quality \
             sign-off cannot sign off without it — write it into the pull request body.",
            only.describe()
        )),
        many => GateStatus::Failed(format!(
            "This change carries neither half of the Product artifact: it states no {}. \
             Quality sign-off cannot sign off without them — write them into the pull \
             request body.",
            many.iter()
                .map(|artifact| artifact.describe())
                .collect::<Vec<_>>()
                .join(", and no ")
        )),
    }
}

fn record(artifact: Artifact, problem_written: &mut bool, bar_written: &mut bool) {
    match artifact {
        Artifact::WrittenProblem => *problem_written = true,
        Artifact::DoneWhenBar => *bar_written = true,
    }
}

fn already(artifact: Artifact, problem_written: bool, bar_written: bool) -> bool {
    match artifact {
        Artifact::WrittenProblem => problem_written,
        Artifact::DoneWhenBar => bar_written,
    }
}

// ---------------------------------------------------------------------------
// Headings
// ---------------------------------------------------------------------------

/// The depth a bold-only label sits at when it OPENS a section.
///
/// A bold label carries no depth of its own. Two is the weight the rest of a
/// pull request body is written at, so a sibling `## Testing` still ends the
/// section a bold `Done when` opened, while a `### Criteria` under it is still
/// a sub-heading inside it. As a BOUNDARY a bold label needs no depth at all:
/// it is a label opening a different topic, so it always ends the section above
/// it.
const BOLD_DEPTH: usize = 2;

/// A heading line: how deep it sits, whether it is a bold label, which artifact
/// it announces if any, and what it carries on its own line after a colon.
struct Heading<'a> {
    depth: usize,
    bold: bool,
    marker: Option<Artifact>,
    inline: &'a str,
}

/// The heading `trimmed` is, or `None` when it is ordinary writing.
///
/// An issue reference is not a heading: ATX requires whitespace after the
/// hashes, and a bare issue reference is one of the commonest things an author
/// writes instead of a bar. A colon-terminated lead-in is not a heading either
/// — see the module docs.
fn heading_of(trimmed: &str) -> Option<Heading<'_>> {
    let (depth, bold, text) = if trimmed.starts_with('#') {
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes > 6 {
            return None;
        }
        let rest = &trimmed[hashes..];
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return None;
        }
        (hashes, false, rest.trim())
    } else {
        let inner = trimmed.strip_prefix("**")?.strip_suffix("**")?;
        if inner.trim().is_empty() || inner.contains("**") {
            return None;
        }
        (BOLD_DEPTH, true, inner.trim())
    };

    let (label, inline) = match text.split_once(':') {
        Some((label, inline)) => (label, inline.trim()),
        None => (text, ""),
    };

    Some(Heading {
        depth,
        bold,
        marker: marker_of(label),
        inline,
    })
}

/// The artifact `label` announces, or `None`.
///
/// The two English words, in any case and with any whitespace. Which words
/// announce a section is otherwise left open: recognising more of them makes a
/// more forgiving gate and nothing in the specification punishes it.
fn marker_of(label: &str) -> Option<Artifact> {
    match collapsed_lowercase(label).as_str() {
        "problem" => Some(Artifact::WrittenProblem),
        "done when" | "done-when" => Some(Artifact::DoneWhenBar),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Content
// ---------------------------------------------------------------------------

/// Whether `line` is the author's own writing rather than the scaffolding
/// around it.
fn is_content(line: &str) -> bool {
    let visible = visible_text(line);
    if visible.is_empty() {
        return false;
    }

    // Repeatedly, because the markers stack: a bulleted checkbox is `- [ ]`,
    // and a bulleted bulleted checkbox is what a template paste leaves behind.
    // Each pass strictly shortens the line or stops.
    let mut core = visible.as_str();
    loop {
        let stripped = strip_checkbox(strip_bullet(core)).trim();
        if stripped == core {
            break;
        }
        core = stripped;
    }
    if core.is_empty() {
        return false;
    }
    // Nothing but punctuation: a lone dash, an asterisk, an ellipsis, an
    // underscore, a row of question marks.
    if !core.chars().any(char::is_alphanumeric) {
        return false;
    }
    // A lead-in introduces writing; it is not the writing.
    if core.ends_with(':') {
        return false;
    }
    if is_pointer_elsewhere(core) {
        return false;
    }
    if is_deferral(core) {
        return false;
    }
    true
}

/// `line` with the zero-width characters dropped, every other run of
/// whitespace collapsed to one space, and the ends trimmed.
///
/// U+200B and U+FEFF are not `char::is_whitespace`, so a section holding one
/// survives `trim()` while rendering as an empty heading. U+00A0, U+2003 and
/// U+3000 arrive whenever anyone pastes out of a document editor, and next to
/// real words they erase nothing — so they are normalised rather than rejected.
fn visible_text(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut pending_space = false;
    for c in line.chars() {
        if is_zero_width(c) {
            continue;
        }
        if c.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(c);
    }
    out
}

fn is_zero_width(c: char) -> bool {
    matches!(
        c,
        '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}'
    )
}

/// `text` lowercased, with every run of whitespace collapsed to one space.
fn collapsed_lowercase(text: &str) -> String {
    visible_text(text).to_lowercase()
}

/// `text` with one leading list bullet removed.
fn strip_bullet(text: &str) -> &str {
    let t = text.trim_start();
    for bullet in ['-', '*', '+', '•'] {
        if let Some(rest) = t.strip_prefix(bullet)
            && (rest.is_empty() || rest.starts_with(' '))
        {
            return rest.trim_start();
        }
    }
    t
}

/// `text` with one leading task-list checkbox removed.
///
/// A ticked or unticked box in front of a criterion is the commonest spelling
/// of an acceptance bar there is, so the marker is stripped and what follows is
/// judged. An empty box is caught by there being nothing after it, not by the
/// box announcing a deferral.
fn strip_checkbox(text: &str) -> &str {
    let t = text.trim_start();
    for checkbox in ["[ ]", "[x]", "[X]"] {
        if let Some(rest) = t.strip_prefix(checkbox) {
            return rest.trim_start();
        }
    }
    t
}

/// Placeholder tokens an author leaves where the artifact belongs.
///
/// Matched as whole content, or as a token announcing a deferral with a colon
/// or a dash. Never as a prefix: `Navigation`, `Native`, `NAT` and `Wipe` all
/// open with one of these and are ordinary writing.
const DEFERRAL_TOKENS: &[&str] = &["tbd", "tba", "n/a", "na", "todo", "to do", "wip", "xxx"];

/// Whole-content deferrals that share no prefix with the tokens above.
const DEFERRAL_PHRASES: &[&str] = &[
    "see the linked issue",
    "see the linked issues",
    "same as above",
    "same as below",
    "will fill this in later",
    "as discussed in standup",
];

/// Whether `core` defers the artifact instead of stating it.
fn is_deferral(core: &str) -> bool {
    let lowered = core.to_lowercase();
    let base = trim_deferral_trailer(&lowered);
    if base.is_empty() {
        return true;
    }
    if DEFERRAL_PHRASES.contains(&base) {
        return true;
    }
    if is_deferral_token(base) {
        return true;
    }
    // A placeholder with an owner on it is still a placeholder.
    if let Some(head) = base.strip_suffix(')')
        && let Some((token, _)) = head.split_once('(')
        && is_deferral_token(token.trim())
    {
        return true;
    }
    announces_a_deferral(base)
}

fn is_deferral_token(base: &str) -> bool {
    DEFERRAL_TOKENS.contains(&base)
}

/// `text` with the punctuation a human trails a placeholder with removed.
fn trim_deferral_trailer(text: &str) -> &str {
    text.trim_end_matches(|c: char| {
        c.is_whitespace() || matches!(c, '.' | '!' | '?' | ':' | '-' | '–' | '—' | '…' | '_')
    })
}

/// Whether `base` opens with a deferral token that announces itself.
///
/// The separator is what tells a placeholder followed by a colon from the same
/// word used in a sentence. A colon or a dash announces a deferral; a space is
/// ordinary writing, and rejecting it costs an author a real acceptance bar.
fn announces_a_deferral(base: &str) -> bool {
    for token in DEFERRAL_TOKENS {
        let Some(rest) = base.strip_prefix(token) else {
            continue;
        };
        // The token has to be a whole word: this is not the opening of
        // `navigation`.
        if rest.starts_with(|c: char| c.is_alphanumeric()) {
            continue;
        }
        if rest.trim_start().starts_with([':', '-', '–', '—', '…']) {
            return true;
        }
    }
    false
}

/// Whether `core` is nothing but a reference to somewhere else.
///
/// The reviewer, the auditor and the scorecard all read this body and none of
/// them follows the link, so a section whose whole content is a pointer has not
/// produced the artifact. A pointer BESIDE real content erases nothing: a bar
/// that cites the panel it will be read on is a better bar, not a deferred one.
fn is_pointer_elsewhere(core: &str) -> bool {
    let lowered = core.to_lowercase();
    let rest = lowered.strip_prefix("see ").unwrap_or(&lowered).trim();
    let mut tokens = rest.split_whitespace();
    let (Some(only), None) = (tokens.next(), tokens.next()) else {
        return false;
    };
    is_pointer_token(only)
}

fn is_pointer_token(token: &str) -> bool {
    if let Some(number) = token.strip_prefix('#') {
        return !number.is_empty() && number.chars().all(|c| c.is_ascii_digit());
    }
    if token.contains("://") {
        return true;
    }
    match token.split_once('/') {
        Some((host, _)) => host.contains('.'),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Template comments
// ---------------------------------------------------------------------------

/// `body` with the contents of every HTML comment blanked, line structure
/// preserved.
///
/// A template prompt is scaffolding whether it was written on one line or over
/// three, and a prompt left above the text the author typed under it erases
/// nothing — so the comment is removed and what is left is judged, rather than
/// the section being rejected for holding one.
fn without_html_comments(body: &str) -> Cow<'_, str> {
    if !body.contains("<!--") {
        return Cow::Borrowed(body);
    }

    let mut out = String::with_capacity(body.len());
    let mut rest = body;
    loop {
        let Some(open) = rest.find("<!--") else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..open]);
        let comment = &rest[open..];
        let end = comment.find("-->").map(|i| i + 3).unwrap_or(comment.len());
        for c in comment[..end].chars() {
            out.push(if c == '\n' { '\n' } else { ' ' });
        }
        rest = &comment[end..];
    }
    Cow::Owned(out)
}
