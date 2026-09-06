//! Finite syntax evidence for complete reserved-invalid literal values only.
//! This neither evaluates expressions nor establishes runtime isolation.

use super::CLEARTEXT_SCHEME;
use syn::{Expr, Lit, Pat, Stmt};

/// Keep the original byte offsets: comment-stripped prefixes cannot prove a
/// value is complete. Unsupported syntax never grants this exception.
pub(super) fn complete_value_at(original: &str, scheme_offset: usize) -> bool {
    let trimmed = original.trim_end();
    let expression = trimmed.strip_suffix(',').unwrap_or(trimmed);
    if let Ok(expr) = syn::parse_str::<Expr>(expression) {
        return supported_value(&expr, original, scheme_offset) == Some(true);
    }
    match syn::parse_str::<Stmt>(original) {
        Ok(Stmt::Local(local)) if local.attrs.is_empty() => {
            let Pat::Ident(name) = local.pat else {
                return false;
            };
            if !name.attrs.is_empty() || name.subpat.is_some() {
                return false;
            }
            local.init.is_some_and(|init| {
                init.diverge.is_none()
                    && supported_value(&init.expr, original, scheme_offset) == Some(true)
            })
        }
        Ok(Stmt::Expr(expr, _)) => supported_value(&expr, original, scheme_offset) == Some(true),
        _ => false,
    }
}

/// The allowlist is deliberately finite: plain strings, simple assignments,
/// tuples/parentheses, Some(value), and literal.to_owned(). These spellings
/// preserve the historical inert fixture values without trusting arbitrary
/// call wrappers, concatenation, interpolation, or raw/escaped strings.
/// None means unsupported syntax, including any unsupported tuple member.
fn supported_value(expr: &Expr, original: &str, offset: usize) -> Option<bool> {
    match expr {
        Expr::Lit(value) if value.attrs.is_empty() => {
            let Lit::Str(literal) = &value.lit else {
                return None;
            };
            let range = literal.span().byte_range();
            let spelling = original.get(range.clone())?;
            let value = spelling.strip_prefix('"')?.strip_suffix('"')?;
            if value
                .bytes()
                .any(|b| !b.is_ascii() || b.is_ascii_control() || b == b'\\')
            {
                return None;
            }
            Some(
                range.start + 1 == offset
                    && !value.bytes().any(|b| b.is_ascii_whitespace())
                    && value
                        .strip_prefix(CLEARTEXT_SCHEME)
                        .is_some_and(reserved_authority),
            )
        }
        Expr::Assign(assign) if assign.attrs.is_empty() => {
            let Expr::Path(name) = assign.left.as_ref() else {
                return None;
            };
            if !name.attrs.is_empty() || name.qself.is_some() || name.path.get_ident().is_none() {
                return None;
            }
            supported_value(&assign.right, original, offset)
        }
        Expr::Paren(paren) if paren.attrs.is_empty() => {
            supported_value(&paren.expr, original, offset)
        }
        Expr::Tuple(tuple) if tuple.attrs.is_empty() => {
            tuple.elems.iter().try_fold(false, |found, element| {
                supported_value(element, original, offset).map(|current| found || current)
            })
        }
        Expr::Call(call) if call.attrs.is_empty() && call.args.len() == 1 => {
            let Expr::Path(name) = call.func.as_ref() else {
                return None;
            };
            if !name.attrs.is_empty() || name.qself.is_some() || !name.path.is_ident("Some") {
                return None;
            }
            supported_value(call.args.first()?, original, offset)
        }
        Expr::MethodCall(call)
            if call.attrs.is_empty()
                && call.method == "to_owned"
                && call.turbofish.is_none()
                && call.args.is_empty()
                && matches!(call.receiver.as_ref(), Expr::Lit(_)) =>
        {
            supported_value(&call.receiver, original, offset)
        }
        _ => None,
    }
}

/// RFC 6761 section 6.4 reserves invalid and its DNS descendants as nonexistent.
/// The caller has already established the entire whitespace-free literal;
/// only URI delimiters, never source quotes/comments, can end its authority.
fn reserved_authority(after_scheme: &str) -> bool {
    let authority = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let host = if let Some((host, port)) = authority.split_once(':') {
        if port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || port.parse::<u16>().is_err()
        {
            return false;
        }
        host
    } else {
        authority
    };
    let host = host.strip_suffix('.').unwrap_or(host);
    if host.len() > 253
        || !host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.as_bytes()[0].is_ascii_alphanumeric()
                && label.as_bytes()[label.len() - 1].is_ascii_alphanumeric()
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return false;
    }
    host.rsplit('.')
        .next()
        .is_some_and(|label| label.eq_ignore_ascii_case("invalid"))
}
