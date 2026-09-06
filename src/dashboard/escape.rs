//! HTML escaping for the dashboard.
//!
//! No escaping existed anywhere in this codebase (a repo-wide grep for
//! `escape_html`/`html_escape` returned zero hits), while `panel_formatters`
//! interpolates GitHub-controlled values — repository names, PR titles, account
//! ids, guard summaries — into HTML with bare `format!`.
//!
//! That is stored XSS into the dashboard, which shares an origin with the
//! unauthenticated `/api/*` control surface. Binding to loopback does not
//! mitigate it: the victim's browser is on loopback.
//!
//! Invariant I4: untrusted input is escaped before rendering.

/// Escapes text for interpolation into HTML element content or a quoted
/// attribute value.
///
/// Escapes `&` first so already-escaped sequences are not double-decoded, then
/// `<`/`>` for element context and `"`/`'` for attribute context.
pub fn html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
    out
}

/// Escapes and truncates, for fields with no useful length bound (PR titles,
/// guard summaries). Truncation happens on a char boundary, before escaping, so
/// an entity is never cut in half.
pub fn html_truncated(input: &str, max_chars: usize) -> String {
    let truncated: String = input.chars().take(max_chars).collect();
    let mut s = html(&truncated);
    if input.chars().count() > max_chars {
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutralises_the_pr_title_xss_payload() {
        // The concrete attack: a PR titled with an onerror handler, rendered
        // into the dashboard, executing against the unauthenticated /api/*.
        let payload = r#"<img src=x onerror="fetch('/api/drain',{method:'POST'})">"#;
        let out = html(payload);
        assert!(!out.contains('<'), "raw '<' survived: {out}");
        assert!(!out.contains('>'), "raw '>' survived: {out}");
        assert!(!out.contains('"'), "raw quote survived: {out}");
        assert!(out.contains("&lt;img"));
    }

    #[test]
    fn escapes_ampersand_first_so_entities_are_not_double_decoded() {
        assert_eq!(html("&lt;"), "&amp;lt;");
        assert_eq!(html("a & b"), "a &amp; b");
    }

    #[test]
    fn escapes_attribute_breaking_characters() {
        assert_eq!(html(r#"" onload="x"#), "&quot; onload=&quot;x");
        assert_eq!(html("' onload='x"), "&#x27; onload=&#x27;x");
    }

    #[test]
    fn leaves_ordinary_text_intact() {
        assert_eq!(html("oyatie/anvil"), "oyatie/anvil");
        assert_eq!(html("feat: add widget (#123)"), "feat: add widget (#123)");
    }

    #[test]
    fn truncation_is_char_safe_and_still_escapes() {
        let out = html_truncated("<script>aaaaaaaaaa", 8);
        assert!(out.starts_with("&lt;script"));
        assert!(out.ends_with('…'));
        assert!(!out.contains('<'));
        // Multibyte input must not panic or split a character.
        assert!(!html_truncated("日本語テキストです", 3).contains('\u{FFFD}'));
    }
}
