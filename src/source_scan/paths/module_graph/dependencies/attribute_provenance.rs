//! Conservative provenance for syntax-transforming attributes.

use quote::ToTokens;
use std::collections::BTreeMap;
use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, Path};

use super::symbols::Symbols;
use crate::source_scan::cfg::{Truth, truth_when_test_is_false};

mod helpers;

#[derive(Clone, Copy)]
pub(super) struct Provenance<'a> {
    pub(super) symbols: &'a Symbols,
    pub(super) scope: &'a [String],
    pub(super) aliases: &'a BTreeMap<String, Vec<Vec<String>>>,
    pub(super) macros: &'a BTreeMap<String, Vec<proc_macro2::TokenStream>>,
}

pub(super) fn uncertainty(attribute: &Attribute, provenance: Provenance<'_>) -> Option<String> {
    check_meta(&attribute.meta, &provenance)
}

pub(super) fn referenced_tokens(
    attribute: &Attribute,
    provenance: Provenance<'_>,
) -> Result<Vec<proc_macro2::TokenStream>, String> {
    if uncertainty(attribute, provenance).is_some() {
        return Ok(Vec::new());
    }
    tokens_for_meta(&attribute.meta, &provenance)
}

fn tokens_for_meta(
    meta: &Meta,
    provenance: &Provenance<'_>,
) -> Result<Vec<proc_macro2::TokenStream>, String> {
    let name = path_name(meta.path());
    match name.as_str() {
        "cfg_attr" => {
            let Meta::List(list) = meta else {
                return Err("malformed cfg_attr cannot be measured".to_owned());
            };
            let nested = list
                .parse_args_with(Punctuated::<Meta, syn::token::Comma>::parse_terminated)
                .map_err(|_| "cfg_attr contents cannot be measured".to_owned())?;
            if truth_when_test_is_false(&nested[0]) == Truth::AlwaysFalse {
                return Ok(Vec::new());
            }
            let mut tokens = Vec::new();
            for nested in nested.iter().skip(1) {
                tokens.extend(tokens_for_meta(nested, provenance)?);
            }
            Ok(tokens)
        }
        "serde" => helpers::analyze(meta, helpers::Kind::Serde),
        "command" | "arg" | "value_enum" | "subcommand" => {
            helpers::analyze(meta, helpers::Kind::Clap)
        }
        _ if is_audited_attribute(meta, provenance) => helpers::analyze(
            meta,
            audited_attribute_kind(&name)
                .ok_or_else(|| format!("audited attribute {name} has no argument policy"))?,
        ),
        _ => Ok(Vec::new()),
    }
}

fn check_meta(meta: &Meta, provenance: &Provenance<'_>) -> Option<String> {
    let name = path_name(meta.path());
    match name.as_str() {
        "cfg_attr" => check_cfg_attr(meta, provenance),
        "derive" => check_derive(meta, provenance),
        // Compiler and inert tool attributes do not synthesize new source.
        "allow"
        | "expect"
        | "warn"
        | "deny"
        | "forbid"
        | "cfg"
        | "test"
        | "path"
        | "doc"
        | "repr"
        | "default"
        | "non_exhaustive"
        | "inline"
        | "cold"
        | "must_use"
        | "deprecated"
        | "link"
        | "link_name"
        | "no_mangle"
        | "export_name"
        | "used"
        | "global_allocator"
        | "panic_handler"
        | "alloc_error_handler"
        | "track_caller"
        | "target_feature"
        | "instruction_set"
        | "naked"
        | "recursion_limit"
        | "type_length_limit"
        | "windows_subsystem"
        | "crate_name"
        | "crate_type"
        | "feature"
        | "no_std"
        | "no_main"
        | "macro_export"
        | "macro_use" => None,
        "serde" if audited_helper_attribute("serde", "serde", provenance) => {
            helpers::analyze(meta, helpers::Kind::Serde).err()
        }
        "command" | "arg" | "value_enum" | "subcommand"
            if audited_helper_attribute("clap", &name, provenance) =>
        {
            helpers::analyze(meta, helpers::Kind::Clap).err()
        }
        _ if is_audited_attribute(meta, provenance) => audited_attribute_kind(&name)
            .ok_or_else(|| format!("audited attribute {name} has no argument policy"))
            .and_then(|kind| helpers::analyze(meta, kind))
            .err(),
        _ if name.starts_with("clippy::")
            || name.starts_with("rustfmt::")
            || name.starts_with("diagnostic::")
            || name.starts_with("coverage::") =>
        {
            None
        }
        _ => Some(format!(
            "attribute #[{}] may expand production syntax and has no audited provenance",
            meta.to_token_stream()
        )),
    }
}

fn is_audited_attribute(meta: &Meta, provenance: &Provenance<'_>) -> bool {
    provenance.symbols.is_audited_attribute(
        &meta
            .path()
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>(),
        provenance.scope,
        provenance.aliases,
    )
}

fn audited_attribute_kind(name: &str) -> Option<helpers::Kind> {
    match name {
        "async_trait" | "async_trait::async_trait" => Some(helpers::Kind::AsyncTrait),
        "tokio::main" => Some(helpers::Kind::Tokio),
        _ => None,
    }
}

fn audited_helper_attribute(
    package_alias: &str,
    attribute_name: &str,
    provenance: &Provenance<'_>,
) -> bool {
    provenance
        .symbols
        .has_audited_package_alias(package_alias, package_alias)
        && !derive_may_be_shadowed(attribute_name, provenance)
        && !provenance
            .symbols
            .binding_declared(provenance.scope, package_alias)
}

fn check_cfg_attr(meta: &Meta, provenance: &Provenance<'_>) -> Option<String> {
    let Meta::List(list) = meta else {
        return Some("malformed cfg_attr cannot be measured".to_owned());
    };
    let nested = match list.parse_args_with(Punctuated::<Meta, syn::token::Comma>::parse_terminated)
    {
        Ok(nested) => nested,
        Err(_) => return Some("cfg_attr contents cannot be measured".to_owned()),
    };
    if nested.len() < 2 {
        return Some("cfg_attr does not name a conditional attribute".to_owned());
    }
    if truth_when_test_is_false(&nested[0]) == Truth::AlwaysFalse {
        return None;
    }
    nested
        .iter()
        .skip(1)
        .find_map(|meta| check_meta(meta, provenance))
}

fn check_derive(meta: &Meta, provenance: &Provenance<'_>) -> Option<String> {
    let Meta::List(list) = meta else {
        return Some("malformed derive cannot be measured".to_owned());
    };
    let derives =
        match list.parse_args_with(Punctuated::<Path, syn::token::Comma>::parse_terminated) {
            Ok(derives) => derives,
            Err(_) => return Some("derive list cannot be measured".to_owned()),
        };
    const BUILTIN: &[&str] = &[
        "Clone",
        "Copy",
        "Debug",
        "Default",
        "Eq",
        "Hash",
        "Ord",
        "PartialEq",
        "PartialOrd",
    ];
    derives.iter().find_map(|derive| {
        let name = path_name(derive);
        let segments = derive
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        let audited_external = derive.leading_colon.is_none()
            && provenance.symbols.is_audited_derive(
                &segments,
                provenance.scope,
                provenance.aliases,
            );
        (!audited_external
            && (derive.leading_colon.is_some()
                || derive.segments.len() != 1
                || !BUILTIN.contains(&name.as_str())
                || derive_may_be_shadowed(&name, provenance)))
        .then(|| format!("derive macro {name} has no locally measurable expansion"))
    })
}

fn derive_may_be_shadowed(name: &str, provenance: &Provenance<'_>) -> bool {
    provenance.macros.contains_key(name)
        || provenance.aliases.contains_key(name)
        || provenance.aliases.contains_key("*")
        || provenance.symbols.macro_shadowed(provenance.scope, name)
        || provenance.symbols.alias_declared(provenance.scope, name)
        || provenance.symbols.glob_imported(provenance.scope)
        || provenance.symbols.unknown_macro_prelude()
}

fn path_name(path: &Path) -> String {
    path.segments
        .iter()
        .map(|part| part.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{Provenance, uncertainty};
    use crate::source_scan::paths::module_graph::dependencies::symbols::Symbols;

    fn reason(source: &str, symbols: &Symbols) -> Option<String> {
        let item: syn::ItemStruct = syn::parse_str(source).expect("attribute fixture");
        uncertainty(
            &item.attrs[0],
            Provenance {
                symbols,
                scope: &[],
                aliases: &BTreeMap::new(),
                macros: &BTreeMap::new(),
            },
        )
    }

    #[test]
    fn imported_or_globbed_builtin_derive_names_are_not_assumed_builtin() {
        let mut exact = Symbols::default();
        exact.add_use(&[], &syn::parse_str("use evil::Debug;").unwrap());
        assert!(reason("#[derive(Debug)] struct S;", &exact).is_some());

        let mut glob = Symbols::default();
        glob.add_use(&[], &syn::parse_str("use evil::*;").unwrap());
        assert!(reason("#[derive(Clone)] struct S;", &glob).is_some());
    }

    #[test]
    fn definitely_inactive_cfg_attr_does_not_create_production_uncertainty() {
        let symbols = Symbols::default();
        assert!(reason("#[cfg_attr(test, evil::inject)] struct S;", &symbols).is_none());
        assert!(reason("#[cfg_attr(unix, evil::inject)] struct S;", &symbols).is_some());
    }
}
