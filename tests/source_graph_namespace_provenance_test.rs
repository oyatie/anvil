//! Namespace and lexical-lifetime provenance for source-graph resolution.

use std::fs;
use std::path::Path;
use std::process::Command;

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn tree(root_source: &str, body: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    write(root.path().join("src/lib.rs"), root_source);
    write(root.path().join("src/account_pool.rs"), "pub fn thing(){}");
    write(
        root.path().join("src/api_contract_guard.rs"),
        "pub fn check(){}",
    );
    write(root.path().join("src/brand_absence.rs"), body);
    root
}

fn graph(root: &Path) -> std::collections::BTreeSet<String> {
    anvil::source_scan::paths::production_module_dependencies(root)
        .unwrap()
        .remove("brand_absence")
        .unwrap()
}

fn cargo_check(root: &Path) {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["check", "--offline"])
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "fixture must compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_root_extern_crate_self_alias_is_visible_in_an_external_child_module() {
    let root = tree(
        "extern crate self as legacy; mod account_pool; mod api_contract_guard; mod brand_absence;",
        "pub fn run(){ legacy::account_pool::thing(); }",
    );
    assert!(graph(root.path()).contains("account_pool/thing"));
}

#[test]
fn absolute_std_include_is_not_blessed_when_std_is_a_crate_alias() {
    let root = tree(
        "#![no_std]\nextern crate self as std; #[macro_export] macro_rules! include {()=>{ crate::account_pool::thing() }} mod account_pool; mod api_contract_guard; mod brand_absence;",
        "pub fn run(){ ::std::include!(); }",
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("a rebound absolute std namespace must not be treated as builtin");
    assert!(reason.contains("include provenance"), "{reason}");
}

#[test]
fn a_value_only_import_cannot_hide_an_extern_crate_namespace() {
    for body in [
        "fn helper(){} use self::helper as dep; pub fn run(){ dep::account_pool::thing(); }",
        "use core::mem::drop as dep; pub fn run(){ dep::account_pool::thing(); }",
        "pub fn run(){ use core::mem::drop as dep; dep::account_pool::thing(); }",
    ] {
        let root = tree(
            "extern crate self as dep; mod account_pool; mod api_contract_guard; mod brand_absence;",
            body,
        );
        assert!(graph(root.path()).contains("account_pool/thing"));
    }
}

#[test]
fn a_block_local_module_does_not_shadow_a_sibling_function() {
    let root = tree(
        "extern crate self as dep_alias; mod account_pool; mod api_contract_guard; mod brand_absence;",
        "fn f(){ mod dep_alias { pub fn safe(){} } dep_alias::safe(); } pub fn g(){ dep_alias::account_pool::thing(); }",
    );
    assert!(graph(root.path()).contains("account_pool/thing"));
}

#[test]
fn nested_unknown_macro_expansion_is_not_silently_discarded() {
    let root = tree(
        "mod account_pool; mod api_contract_guard; mod brand_absence;",
        "macro_rules! wrapper {()=>{ external_proc!() }} pub fn run(){ wrapper!(); }",
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("an unknown nested expansion can synthesize source");
    assert!(reason.contains("external_proc"), "{reason}");
}

#[test]
fn block_imports_do_not_cross_a_nested_module_boundary() {
    let root = tree(
        "extern crate self as dep; mod account_pool; mod api_contract_guard; mod brand_absence;",
        "mod safe{} fn f(){ use super::brand_absence::safe as dep; mod nested { fn g(){ dep::account_pool::thing(); } } }",
    );
    assert!(graph(root.path()).contains("account_pool/thing"));
}

#[test]
fn block_modules_from_expression_includes_do_not_escape_their_block() {
    for nested in [false, true] {
        let root = tree(
            "extern crate self as dep_alias; mod account_pool; mod api_contract_guard; mod brand_absence;",
            "fn f(){ include!(\"local.inc\"); } pub fn g(){ dep_alias::account_pool::thing(); }",
        );
        let included = if nested {
            "{ include!(\"nested.inc\"); }"
        } else {
            "{ mod dep_alias { pub fn safe(){} } dep_alias::safe(); }"
        };
        write(root.path().join("src/local.inc"), included);
        if nested {
            write(
                root.path().join("src/nested.inc"),
                "{ mod dep_alias { pub fn safe(){} } dep_alias::safe(); }",
            );
        }
        assert!(graph(root.path()).contains("account_pool/thing"));
    }
}

#[test]
fn one_root_include_is_measured_in_each_distinct_lexical_context() {
    let root = tree(
        "mod account_pool; mod api_contract_guard; \
         fn safe(){ macro_rules! invoke {()=>{ crate::api_contract_guard::check() }} \
             let _ = include!(\"shared.inc\"); } \
         fn dangerous(){ macro_rules! invoke {()=>{ crate::account_pool::thing() }} \
             let _ = include!(\"shared.inc\"); }",
        "",
    );
    write(
        root.path().join("src/shared.inc"),
        "{ mod nested { pub fn run(){ invoke!(); } } nested::run() }",
    );
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    );
    cargo_check(root.path());

    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["nested"].contains("api_contract_guard/check"),
        "{graph:?}"
    );
    assert!(graph["nested"].contains("account_pool/thing"), "{graph:?}");
}
