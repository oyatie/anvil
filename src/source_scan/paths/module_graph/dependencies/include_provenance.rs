use std::collections::BTreeMap;

use super::imports::ident_name;
use super::symbols::{ResolvedPath, Symbols};

pub(super) enum IncludeProvenance {
    Builtin,
    Ambiguous,
    Other,
}

pub(super) fn classify_include(
    invocation: &syn::Macro,
    symbols: &Symbols,
    scope: &[String],
    local_aliases: &BTreeMap<String, Vec<Vec<String>>>,
    local_macros: &BTreeMap<String, Vec<proc_macro2::TokenStream>>,
) -> IncludeProvenance {
    let segments = invocation
        .path
        .segments
        .iter()
        .map(|segment| ident_name(&segment.ident))
        .collect::<Vec<_>>();
    classify_include_segments(
        &segments,
        invocation.path.leading_colon.is_some(),
        symbols,
        scope,
        local_aliases,
        local_macros,
    )
}

pub(super) fn classify_include_segments(
    segments: &[String],
    _leading_colon: bool,
    symbols: &Symbols,
    scope: &[String],
    local_aliases: &BTreeMap<String, Vec<Vec<String>>>,
    local_macros: &BTreeMap<String, Vec<proc_macro2::TokenStream>>,
) -> IncludeProvenance {
    let binding = segments.first().map(String::as_str).unwrap_or_default();
    let shadowed_macro =
        local_macros.contains_key(binding) || symbols.macro_shadowed(scope, binding);
    let imported_macro_possible = local_aliases.contains_key(binding)
        || symbols.alias_declared(scope, binding)
        || local_aliases.contains_key("*")
        || symbols.glob_imported(scope)
        || symbols.unknown_macro_prelude();
    let namespace_shadow = segments.len() > 1
        && matches!(binding, "std" | "core")
        && (local_aliases.contains_key(binding) || symbols.binding_declared(scope, binding));
    let resolved = symbols.resolve(segments, scope, local_aliases);
    let builtin = resolved.iter().filter(|path| is_builtin(path)).count();
    if segments == ["include"] && !shadowed_macro && !imported_macro_possible {
        return IncludeProvenance::Builtin;
    }
    if builtin > 0 && builtin == resolved.len() && !shadowed_macro && !namespace_shadow {
        return IncludeProvenance::Builtin;
    }
    if builtin > 0
        || namespace_shadow
        || (segments == ["include"] && (shadowed_macro || imported_macro_possible))
        || (resolved.is_empty() && symbols.binding_declared(scope, binding))
    {
        return IncludeProvenance::Ambiguous;
    }
    IncludeProvenance::Other
}

fn is_builtin(path: &ResolvedPath) -> bool {
    !path.local
        && matches!(
            path.segments.as_slice(),
            [root, name] if matches!(root.as_str(), "std" | "core") && name == "include"
        )
}
