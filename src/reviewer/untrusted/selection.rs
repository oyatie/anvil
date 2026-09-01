//! Allocation-bounded selection and delimiter neutralisation.

use super::UntrustedLabel;

#[derive(Clone, Copy)]
pub(super) enum Selection {
    Leading,
    Trailing,
    HeadAndTail,
}

/// Wraps contributor text in explicit delimiters carrying the channel's
/// standing instruction.
///
/// This is the uncapped compatibility primitive. Model prompt sites use
/// `Untrusted::render`, which also bounds the segment.
pub fn fence_untrusted(label: UntrustedLabel, content: &str) -> String {
    fence_neutralised(label, label.label(), &neutralise_delimiters(content))
}

/// The fence itself, over content whose markers are already defused.
pub(super) fn fence_neutralised(label: UntrustedLabel, name: &str, neutralised: &str) -> String {
    format!(
        "{}\nBEGIN UNTRUSTED {name}\n{neutralised}\nEND UNTRUSTED {name}",
        label.standing_instruction()
    )
}

/// The marker word the fence is built from. Neutralised wherever it appears in
/// untrusted content.
const FENCE_MARKER: &str = "untrusted";

/// What a quoted marker becomes. Chosen to be self-explaining in the prompt.
const FENCE_MARKER_QUOTED: &str = "_QUOTED_BY_THE_PR_AUTHOR";

/// Defuses every occurrence of the fence marker, case-insensitively.
pub(super) fn neutralise_delimiters(content: &str) -> String {
    let mut out = String::with_capacity(neutralised_len(content));
    let mut cursor = 0usize;
    let mut copied_through = 0usize;
    let bytes = content.as_bytes();
    let marker = FENCE_MARKER.as_bytes();
    while cursor + marker.len() <= bytes.len() {
        let end = cursor + marker.len();
        if bytes[cursor..end].eq_ignore_ascii_case(marker) {
            // A matching byte is ASCII, so both indices are UTF-8 boundaries.
            out.push_str(&content[copied_through..end]);
            out.push_str(FENCE_MARKER_QUOTED);
            cursor = end;
            copied_through = end;
        } else {
            cursor += 1;
        }
    }
    out.push_str(&content[copied_through..]);
    out
}

/// Size of the fully quoted representation, without materialising it.
pub(super) fn neutralised_len(content: &str) -> usize {
    let bytes = content.as_bytes();
    let marker = FENCE_MARKER.as_bytes();
    let mut cursor = 0usize;
    let mut occurrences = 0usize;
    while cursor + marker.len() <= bytes.len() {
        let end = cursor + marker.len();
        if bytes[cursor..end].eq_ignore_ascii_case(marker) {
            occurrences += 1;
            cursor = end;
        } else {
            cursor += 1;
        }
    }
    content
        .len()
        .saturating_add(occurrences.saturating_mul(FENCE_MARKER_QUOTED.len()))
}

/// Selects a bounded original prefix, quotes its markers, then enforces the
/// rendered byte budget. A suffix cut cannot remove an inserted quote while
/// retaining both the marker before it and the delimiter label after it.
pub(super) fn neutralised_leading_excerpt(content: &str, budget: usize) -> String {
    let selected = leading_bytes(content, budget);
    let neutralised = neutralise_delimiters(selected);
    leading_bytes(&neutralised, budget).to_string()
}

/// Tail counterpart of [`neutralised_leading_excerpt`]. A prefix cut cannot
/// retain both sides of a marker whose inserted quote it removed.
pub(super) fn neutralised_trailing_excerpt(content: &str, budget: usize) -> String {
    let selected = trailing_bytes(content, budget);
    let neutralised = neutralise_delimiters(selected);
    trailing_bytes(&neutralised, budget).to_string()
}

/// Builds the trusted declaration from the original and fully quoted lengths.
pub(super) fn truncation_notice(
    measured: usize,
    embedded: usize,
    max: usize,
    what: &str,
    selection: Selection,
) -> String {
    let grown = if embedded == measured {
        String::new()
    } else {
        format!(" ({embedded} bytes once the fence markers it quotes are defused)")
    };
    let shown = match selection {
        Selection::Leading => "Only the leading portion is shown below",
        Selection::Trailing => "Only the trailing portion is shown below",
        Selection::HeadAndTail => "Only leading and trailing portions are shown below",
    };
    format!(
        "[TRUNCATED: the {what} is {measured} bytes{grown}, over the {max}-byte prompt cap. \
         {shown}; the omitted portion was NOT provided and has NOT been reviewed. \
         Do not report on what you were not shown.]\n"
    )
}

fn leading_bytes(content: &str, max: usize) -> &str {
    let mut end = max.min(content.len());
    while end > 0 && !content.is_char_boundary(end) {
        end -= 1;
    }
    &content[..end]
}

fn trailing_bytes(content: &str, max: usize) -> &str {
    let mut start = content.len().saturating_sub(max);
    while start < content.len() && !content.is_char_boundary(start) {
        start += 1;
    }
    &content[start..]
}
