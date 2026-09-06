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

#[test]
fn clap_generated_names_require_known_lexical_bindings() {
    let mut symbols = Symbols::default();
    symbols.add_audited_derive_crate("clap", "clap");
    assert!(reason("#[derive(clap::Parser)] struct Args;", &symbols).is_none());
    symbols.add_use(&[], &syn::parse_str("use other::format;").unwrap());
    assert!(reason("#[derive(clap::Parser)] struct Args;", &symbols).is_some());
}

#[test]
fn serde_default_generated_facade_requires_its_actual_extern_binding() {
    let mut symbols = Symbols::default();
    symbols.add_audited_derive_crate("codec", "serde");
    assert!(reason("#[derive(codec::Serialize)] struct Record;", &symbols).is_some());
    symbols.add_audited_derive_crate("serde", "serde");
    assert!(reason("#[derive(codec::Serialize)] struct Record;", &symbols).is_none());
}
