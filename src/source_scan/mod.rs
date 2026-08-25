//! Reading Rust source as CODE, with its commentary and literal bodies removed.
//!
//! # Why this is one function and not eight
//!
//! Eight implementations of "strip commentary before scanning" existed in this
//! tree under four different behaviours and one name: a four-line filter that
//! drops whole `//` lines, a per-line truncation with a crude quote guard, a
//! line-wise lexer that tracks string literals, and this -- a whole-source state
//! machine that also handles block comments, escapes, and literals spanning
//! lines.
//!
//! That is worse than duplication. A reader seeing `code_only` assumes the
//! strong one and may be handed the weakest, and the failure is silent: a scan
//! that reads a doc comment as code reports a defect that is not there, and one
//! that misses a construct reports nothing at all. Both happened. The
//! line-wise variant could not see that `both_sides(..)` appeared inside a
//! string spanning several lines, so a gate accused its own ledger's prose of
//! being a call site.
//!
//! # Offsets are preserved
//!
//! Everything removed is replaced by spaces rather than deleted, so a byte
//! offset into the result is the same offset in the original. A scanner can
//! report a line and column from it without a second pass.
//!
//! # Two needs, two names
//!
//! A scan looking for a literal MARKER -- the `"+++ b/"` a diff parser keys on,
//! the quoted evidence a citation points at -- must still see string literals;
//! stripping them removes the very thing it is looking for. A scan looking for
//! a construct must NOT see them, or a test fixture quoting `unwrap()` reads as
//! a call to it.
//!
//! Those are different questions and they get different functions:
//! [`code_only`] removes commentary and literal bodies; [`without_commentary`]
//! removes commentary and keeps them. Both preserve offsets. Calling one
//! `code_only` and leaving the caller to guess which behaviour it has is how
//! nine spellings drifted into four behaviours under one name.
//!
//! # What it does not model
//!
//! Raw strings (`r#"..."#`) and a quote inside a character literal. Both are
//! stated rather than implied, because the point of a mechanism is to not
//! overclaim its coverage: either could hide a hit, and neither can invent one.

pub fn code_only(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if c == '\n' {
            in_line_comment = false;
            out.push('\n');
            continue;
        }
        if in_line_comment {
            out.push(' ');
            continue;
        }
        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
                out.push_str("  ");
            } else {
                out.push(' ');
            }
            continue;
        }
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
                out.push('"');
                continue;
            }
            out.push(' ');
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push('"');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            in_line_comment = true;
            out.push(' ');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            out.push_str("  ");
            continue;
        }
        out.push(c);
    }
    out
}

/// Rust source with its commentary removed and its string literals INTACT.
///
/// For a scan whose subject is a literal: the `"+++ b/"` a diff parser keys on,
/// the quoted evidence a fidelity citation points at. [`code_only`] would strip
/// exactly what such a scan is looking for -- swapping one for the other took a
/// ratchet's count from nineteen sites to two, silently, because every marker
/// it matches is spelled as a string.
///
/// Offsets are preserved, as in [`code_only`]: removed text becomes spaces.
pub fn without_commentary(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if c == '\n' {
            in_line_comment = false;
            out.push('\n');
            continue;
        }
        if in_line_comment {
            out.push(' ');
            continue;
        }
        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
                out.push_str("  ");
            } else {
                out.push(' ');
            }
            continue;
        }
        if in_str {
            // The body is KEPT. Escapes are still honoured so a `\"` does not
            // end the literal early and leave the scanner reading code as text.
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            out.push(c);
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            in_line_comment = true;
            out.push(' ');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            out.push_str("  ");
            continue;
        }
        out.push(c);
    }
    out
}
