use std::collections::BTreeSet;
use std::path::PathBuf;

use syn::punctuated::Punctuated;
use syn::{Attribute, Meta};

use crate::source_scan::cfg::{Truth, truth_when_test_is_false};

#[derive(Default)]
pub(super) struct PathOverrides {
    pub(super) certain: BTreeSet<PathBuf>,
    pub(super) possible: BTreeSet<PathBuf>,
}

pub(super) fn path_overrides(attrs: &[Attribute]) -> Result<PathOverrides, String> {
    let mut overrides = PathOverrides::default();
    for attr in attrs {
        if attr.path().is_ident("path") || attr.path().is_ident("cfg_attr") {
            collect_path_meta(&attr.meta, Truth::AlwaysTrue, &mut overrides)?;
        }
    }
    if overrides.certain.len() > 1 {
        return Err("multiple active #[path] module attributes".to_owned());
    }
    Ok(overrides)
}

fn collect_path_meta(
    meta: &Meta,
    activation: Truth,
    overrides: &mut PathOverrides,
) -> Result<(), String> {
    if activation == Truth::AlwaysFalse {
        return Ok(());
    }
    if meta.path().is_ident("path") {
        let Meta::NameValue(value) = meta else {
            return Err("malformed #[path] module attribute".to_owned());
        };
        let syn::Expr::Lit(value) = &value.value else {
            return Err("non-literal #[path] module attribute".to_owned());
        };
        let syn::Lit::Str(value) = &value.lit else {
            return Err("non-string #[path] module attribute".to_owned());
        };
        let target = PathBuf::from(value.value());
        match activation {
            Truth::AlwaysTrue => overrides.certain.insert(target),
            Truth::Maybe => overrides.possible.insert(target),
            Truth::AlwaysFalse => false,
        };
        return Ok(());
    }
    if !meta.path().is_ident("cfg_attr") {
        return Ok(());
    }
    let Meta::List(list) = meta else {
        return Err("malformed #[cfg_attr] module attribute".to_owned());
    };
    let parser = Punctuated::<Meta, syn::token::Comma>::parse_terminated;
    let parts = syn::parse::Parser::parse2(parser, list.tokens.clone())
        .map_err(|error| format!("malformed #[cfg_attr] module attribute: {error}"))?;
    let mut parts = parts.iter();
    let condition = parts
        .next()
        .ok_or_else(|| "empty #[cfg_attr] module attribute".to_owned())?;
    let combined = combine(activation, truth_when_test_is_false(condition));
    for nested in parts {
        collect_path_meta(nested, combined, overrides)?;
    }
    Ok(())
}

fn combine(outer: Truth, inner: Truth) -> Truth {
    match (outer, inner) {
        (Truth::AlwaysFalse, _) | (_, Truth::AlwaysFalse) => Truth::AlwaysFalse,
        (Truth::AlwaysTrue, Truth::AlwaysTrue) => Truth::AlwaysTrue,
        _ => Truth::Maybe,
    }
}
