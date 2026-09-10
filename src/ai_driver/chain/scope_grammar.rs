//! The `pre-commit` hook's declaration grammar, held where the loader can
//! apply it before a declaration ever reaches the hook.

/// The `pre-commit` hook's own declaration grammar, ported clause for clause.
///
/// An approximation of it -- leading `/`,
/// `..`, blank -- and the hook's grammar is strictly larger. Eight forms loaded
/// here and were then refused by the consumer, which does not merely fail: a
/// declaration the hook cannot parse refuses EVERY commit in that run. One
/// form, a prefix containing a newline, did worse and silently split into two
/// prefixes, widening the scope past what a reader of the file would see.
///
/// This mirrors `anvil_scope_literal(.., declaration)` in
/// `src/git_manager/hooks/pre-commit`. The two must not drift, and
/// `a_declaration_the_hook_would_refuse_is_a_load_error` holds the shared
/// fixtures that say they have not.
pub(super) fn scope_prefix_is_writable_by_the_hook(prefix: &str) -> Result<(), &'static str> {
    // The hook does no trimming, so surrounding whitespace is part of the
    // literal and would silently match nothing. Refused loudly here instead.
    if prefix != prefix.trim() {
        return Err("has leading or trailing whitespace, which the hook takes literally");
    }
    // One trailing slash, as the hook strips for a declaration.
    let literal = prefix.strip_suffix('/').unwrap_or(prefix);
    if literal.is_empty() {
        return Err("is empty");
    }
    if literal.starts_with('/') {
        return Err("is absolute; the scope is repository-relative");
    }
    let bytes = literal.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err("looks like a drive-lettered path");
    }
    if literal.contains('\\') {
        return Err("contains a backslash");
    }
    if literal.contains('"') {
        return Err("contains a quote");
    }
    if literal.chars().any(char::is_control) {
        return Err("contains a control byte; a newline would silently split it in two");
    }
    let framed = format!("/{literal}/");
    if framed.contains("//") || framed.contains("/./") || framed.contains("/../") {
        return Err("has an empty, `.` or `..` path component");
    }
    Ok(())
}
