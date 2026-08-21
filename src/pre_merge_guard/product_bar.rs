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
//! gate whose whole subject is named gates with no measurement. The suite
//! already decides the question behaviourally: a change whose bet appears only
//! in a descriptive title is still missing its written problem
//! (`the_bet_and_the_bar_are_written_on_the_change_not_left_to_its_title`), so
//! the title cannot supply either artifact and is not an input. Listed in
//! open_questions as a decision a human can veto.
//!
//! Absence is the defect itself. A change that never wrote a bar has not
//! produced evidence this gate could not read; it produced no bar. That is
//! `Failed`, not `NotMeasured` and not `Warning` — and that holds for a body
//! that uses no heading this gate recognises just as much as for an empty one.
//!
//! Two entry points, deliberately: `missing_artifacts` is the measurement and
//! `judge` is the verdict rendered from it. The split exists so the tests can
//! assert *which* artifact the gate found missing without pattern-matching on
//! the prose of the message, which would forbid the gate from quoting the
//! offending section back at the author.
//!
//! # How the measurement works
//!
//! Three rules, applied in this order, and none of them is a rule about length:
//!
//!   1. **A marker is a heading line.** An ATX heading (`#`, `##`, `###`) or a
//!      line that is nothing but a bold label (`**Done when**`) opens a section
//!      and closes the one above it. Case, depth, a trailing colon, a bold
//!      wrapper and content written on the heading's own line are formatting,
//!      not vocabulary. A line ending in a colon is ordinary technical writing
//!      and never a boundary — `Acceptance criteria:` above a list of bullets is
//!      one of the two commonest shapes a done-when takes.
//!   2. **The section is judged on its content, line by line.** A section holds
//!      the artifact if *any* line in it is content. A leftover checkbox above
//!      three real criteria is a bar; a template prompt left above the author's
//!      own prose is a problem statement. Strip what is not content — bullets,
//!      checkbox markers, HTML comments, invisible characters — and judge what
//!      is left.
//!   3. **A line is content unless it defers.** Empty after stripping, a bare
//!      lead-in label, a deferral (`TBD`, `n/a`, `todo(jason)`, `same as
//!      above`), or a section whose whole content points somewhere else is not
//!      the artifact. Terse is not absent: `- p99 < 5ms` is a checkable
//!      condition and this gate takes it as one, because measuring how much the
//!      author typed accuses the teams who write the sharpest bars.
//!
//! Which words announce a section is deliberately more generous than the two
//! the ADR names: `## Acceptance criteria` and `## Definition of done` are read
//! as a done-when, `## Why` and `## Motivation` as the bet. Recognising more
//! spellings can only admit an author who did the job, and this gate runs on
//! every change in the fleet, none of which shares one template. The list is a
//! floor rather than a fence — a human who wants a narrower or wider vocabulary
//! edits `Artifact::markers` and no test in the suite moves.

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

/// The artifacts in canonical order, which is also declaration order.
const ARTIFACTS: [Artifact; 2] = [Artifact::WrittenProblem, Artifact::DoneWhenBar];

impl Artifact {
    /// The normalised heading labels that announce this artifact's section.
    ///
    /// A floor, not a fence: a heading spelled any of these ways opens the
    /// section, and recognising more of them only widens the set of authors who
    /// are credited with work they actually did.
    fn markers(self) -> &'static [&'static str] {
        match self {
            Artifact::WrittenProblem => &[
                "problem",
                "the problem",
                "problem statement",
                "why",
                "motivation",
            ],
            Artifact::DoneWhenBar => &[
                "done when",
                "acceptance bar",
                "acceptance criteria",
                "acceptance criterion",
                "definition of done",
            ],
        }
    }

    /// How the failure message names this artifact's absence.
    ///
    /// Each phrase names its own artifact and never the other one: the message
    /// is what the author reads, and telling someone to write the section they
    /// already wrote is the same defect as certifying a change that wrote
    /// neither.
    fn absence(self) -> &'static str {
        match self {
            Artifact::WrittenProblem => "no written problem statement",
            Artifact::DoneWhenBar => "no done-when acceptance bar",
        }
    }

    /// What the author has to do about it.
    fn guidance(self) -> &'static str {
        match self {
            Artifact::WrittenProblem => {
                "Write what is wrong and why it matters, on the change itself."
            }
            Artifact::DoneWhenBar => {
                "Write how someone other than the author checks this change is finished, on the \
                 change itself."
            }
        }
    }
}

/// The Product artifacts this change did not produce, in canonical order.
///
/// Empty means the change carries both. This is the measurement; `judge`
/// renders its verdict and its message from it.
pub fn missing_artifacts(pr_body: &str) -> Vec<Artifact> {
    ARTIFACTS
        .into_iter()
        .filter(|artifact| !carries(pr_body, *artifact))
        .collect()
}

/// Judges the Product artifact carried by the change under review.
pub fn judge(pr_body: &str) -> GateStatus {
    let missing = missing_artifacts(pr_body);
    if missing.is_empty() {
        return GateStatus::Passed;
    }

    let named: Vec<&str> = missing.iter().map(|a| a.absence()).collect();
    let guidance: Vec<&str> = missing.iter().map(|a| a.guidance()).collect();
    GateStatus::Failed(format!(
        "The Product seat's artifact is missing from this change: {}. {} Quality sign-off \
         cannot sign off without it.",
        named.join(" and "),
        guidance.join(" ")
    ))
}

/// Whether any section announcing `artifact` holds content.
///
/// One pass over the lines: a heading opens the section it names and closes
/// whatever was open, and the first line of real content settles the question.
/// A marker written twice is treated as one artifact stated in two places —
/// either occurrence carrying content is enough, which is the reading that never
/// accuses an author who wrote the section.
fn carries(body: &str, artifact: Artifact) -> bool {
    let mut inside = false;
    for line in body.lines() {
        match heading(line) {
            Some((label, on_the_marker_line)) => {
                inside = artifact.markers().contains(&label.as_str());
                if inside && is_content(on_the_marker_line) {
                    return true;
                }
            }
            None => {
                if inside && is_content(line) {
                    return true;
                }
            }
        }
    }
    false
}

/// The normalised label of the heading on `line`, plus whatever the author
/// wrote after it on that same line — or `None` if the line is not a heading.
///
/// Two shapes are headings: an ATX heading, and a line that is nothing but a
/// bold label. A colon-terminated line is neither. It is ordinary technical
/// writing, and treating one as a boundary reports a missing acceptance bar for
/// `Acceptance criteria:` above a list of bullets.
fn heading(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim();
    let text = if let Some(after_hashes) = trimmed.strip_prefix('#') {
        let rest = after_hashes.trim_start_matches('#');
        // `#4192` is an issue reference, not a heading: an ATX heading needs
        // whitespace after its hashes.
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return None;
        }
        rest.trim()
    } else {
        bold_label(trimmed)?
    };

    // `## Done when: p99 < 5ms` states the bar on the heading's own line, and an
    // author who writes `## Problem:` has spelled the same marker.
    match text.find(':') {
        Some(i) => Some((normalise_label(&text[..i]), text[i + 1..].trim())),
        None => Some((normalise_label(text), "")),
    }
}

/// The text of a line that is nothing but a bold label.
///
/// `**Done when**` is a marker in this corpus, so a bold-only line naming some
/// other topic closes the section above it. A line with bold *inside* it is
/// prose and is not a heading.
fn bold_label(trimmed: &str) -> Option<&str> {
    let inner = trimmed.strip_prefix("**")?.strip_suffix("**")?.trim();
    if inner.is_empty() || inner.contains("**") {
        return None;
    }
    Some(inner)
}

/// A heading's words, with its formatting removed.
///
/// Case, hyphens, underscores, stray emphasis and repeated whitespace are how
/// markdown is written, not what it says.
fn normalise_label(label: &str) -> String {
    let spelled: String = strip_invisible(label)
        .trim()
        .trim_matches('*')
        .chars()
        .map(|c| if c == '-' || c == '_' { ' ' } else { c })
        .collect();
    spelled
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Whether this line says something a reader could check the change against.
///
/// Deliberately not a length rule in either unit. `- p99<5ms` has no space in
/// it and `- 빌드 통과` is five characters; both are real acceptance criteria,
/// and `TODO: write the acceptance criteria here` is thirty-nine bytes of
/// nothing.
fn is_content(line: &str) -> bool {
    let visible = strip_invisible(line);
    let uncommented = strip_html_comments(&visible);
    let core = strip_list_markers(&uncommented);

    if core.is_empty() {
        return false;
    }
    // A bare lead-in label — `Acceptance criteria:` — is the introduction to
    // the artifact rather than the artifact. It reads as writing when something
    // follows it, and as an empty section when nothing does.
    if core.ends_with(':') {
        return false;
    }
    !defers(core) && !points_elsewhere(core)
}

/// Deferral vocabulary: the ways an author writes "not yet" in a section that
/// was supposed to hold the artifact.
const DEFERRAL_TOKENS: &[&str] = &["tbd", "tba", "n/a", "na", "todo", "to do", "wip", "xxx"];

/// The same thing written as a sentence rather than a token.
const DEFERRAL_PHRASES: &[&str] = &[
    "see the linked issue",
    "same as above",
    "will fill this in later",
    "as discussed in standup",
];

/// Whether this line's whole content is a deferral.
///
/// Token-level, not prefix-level. `- Navigation completes in under 200ms` and
/// `Native TLS was disabled …` both begin with a deferral stem and both are
/// real writing; what makes a deferral a deferral is that it stands alone or
/// announces itself with a separator — `TODO:` and `TBD - ` defer, while
/// `TODO comments are removed from …` is the word used in a sentence.
fn defers(core: &str) -> bool {
    let lowered = core.to_lowercase();
    let bare = trim_trailing_punctuation(&lowered);
    if bare.is_empty() {
        return true;
    }
    // `todo(jason)` is the same deferral with an owner attached.
    let bare = strip_parenthetical(bare);

    DEFERRAL_TOKENS
        .iter()
        .chain(DEFERRAL_PHRASES.iter())
        .any(|token| bare == *token || announces(bare, token))
}

/// Whether `bare` opens with `token` and then announces that nothing follows.
fn announces(bare: &str, token: &str) -> bool {
    match bare.strip_prefix(token) {
        Some(rest) => {
            rest.starts_with(':')
                || rest.starts_with(" -")
                || rest.starts_with(" –")
                || rest.starts_with(" —")
        }
        None => false,
    }
}

/// Whether this line's whole content is a reference to somewhere else.
///
/// The artifact lives on the change under review: the reviewer, the auditor and
/// the scorecard all read this body and none of them follows the link. A
/// pointer *beside* real content is a better bar, not a deferred one — a
/// done-when that names the panel you will read is writing — so this asks
/// whether the content IS a pointer, never whether it contains one.
fn points_elsewhere(core: &str) -> bool {
    let mut tokens = core.split_whitespace().peekable();
    if let Some(first) = tokens.peek()
        && first.trim_end_matches(':').eq_ignore_ascii_case("see")
    {
        tokens.next();
    }
    let rest: Vec<&str> = tokens.collect();
    rest.len() == 1 && is_a_reference(rest[0])
}

/// Whether one token is a URL, a bare host path, or an issue reference.
fn is_a_reference(token: &str) -> bool {
    let token = token.trim_end_matches(['.', ',', ')', ']']);
    if token.starts_with("http://") || token.starts_with("https://") {
        return true;
    }
    if let Some(number) = token.strip_prefix('#') {
        return !number.is_empty() && number.chars().all(|c| c.is_ascii_digit());
    }
    match token.split_once('/') {
        Some((host, _)) => {
            host.contains('.')
                && !host.starts_with('.')
                && !host.ends_with('.')
                && host
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
        }
        None => false,
    }
}

/// `text` without the characters that render as nothing.
///
/// U+200B and U+FEFF are not `char::is_whitespace`, so a section holding one
/// survives `trim()` while rendering as an empty heading. Removing them is the
/// whole of the treatment: the wider spaces an editor pastes in (U+00A0,
/// U+2003, U+3000) already are whitespace, and an invisible character beside
/// real content must not erase the content.
fn strip_invisible(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !matches!(
                c,
                '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{2060}' | '\u{feff}'
            )
        })
        .collect()
}

/// `line` with every HTML comment span removed.
///
/// A pull request template prompt is a comment, and authors overwhelmingly type
/// their answer underneath it rather than deleting it. So the comment is
/// removed and the rest of the line is judged, which reads a prompt on its own
/// as an empty section and a prompt above real prose as real prose.
fn strip_html_comments(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let Some(open) = rest.find("<!--") else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..open]);
        match rest[open..].find("-->") {
            Some(close) => rest = &rest[open + close + "-->".len()..],
            // An unterminated comment swallows the remainder of the line.
            None => return out,
        }
    }
}

/// `line` with its leading bullets, quote markers and checkbox stripped.
///
/// `- [ ] p99 < 5ms` is the single commonest spelling of an acceptance bar
/// there is, and it is the one every pull request template produces: the
/// checkbox marker is punctuation, so it is removed and what the author wrote
/// after it is judged.
fn strip_list_markers(line: &str) -> &str {
    let mut rest = line.trim();
    loop {
        let before = rest;
        rest = rest.trim_start_matches(['-', '*', '+', '•', '>']).trim();
        if let Some(after_box) = strip_checkbox(rest) {
            rest = after_box.trim();
        }
        if rest == before {
            return rest;
        }
    }
}

/// `s` without a leading `[ ]`, `[x]` or `[X]`.
///
/// A box holding anything else is not a checkbox: `[link](url)` is writing.
fn strip_checkbox(s: &str) -> Option<&str> {
    let (inside, after) = s.strip_prefix('[')?.split_once(']')?;
    let inside = inside.trim();
    if inside.is_empty() || inside.eq_ignore_ascii_case("x") {
        Some(after)
    } else {
        None
    }
}

/// `s` without the punctuation a human trails a deferral with.
///
/// Punctuation and whitespace are stripped together rather than in two passes:
/// `???` followed by a space and a dash is one deferral written untidily, and a
/// single pass leaves the question marks behind.
fn trim_trailing_punctuation(s: &str) -> &str {
    s.trim().trim_end_matches(|c: char| {
        c.is_whitespace() || matches!(c, '.' | '!' | '?' | ':' | ';' | ',' | '-' | '_' | '…')
    })
}

/// `todo(jason)` -> `todo`. Anything else is returned unchanged.
fn strip_parenthetical(s: &str) -> &str {
    match s.strip_suffix(')').and_then(|head| head.find('(')) {
        Some(open) => s[..open].trim_end(),
        None => s,
    }
}
