//! Syntax-transforming attributes and macro-prelude uncertainty fail closed.

use std::fs;
use std::path::Path;

fn write(path: impl AsRef<Path>, contents: &str) {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

// These are source/data contracts, not compiled macro expansion demonstrations.
// Metadata uses the existing admitted manifest/lock profile; no fixture is built.
fn admitted_profile(root: &Path) {
    write(root.join("Cargo.toml"), include_str!("../Cargo.toml"));
    write(root.join("Cargo.lock"), include_str!("../Cargo.lock"));
    write(root.join("src/main.rs"), "fn main() {}");
}

fn admitted_tree(body: &str) -> tempfile::TempDir {
    let root = tree(body);
    admitted_profile(root.path());
    anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect("positive control must satisfy the complete admitted profile");
    root
}

fn refuses(root: &Path) -> String {
    let reason = anvil::source_scan::paths::production_module_dependencies(root)
        .expect_err("unproved syntax expansion must prevent measurement");
    assert!(reason.contains("cannot be measured"), "{reason}");
    reason
}

fn tree(body: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("src/lib.rs"),
        "mod account_pool; mod api_contract_guard; mod brand_absence;",
    );
    write(
        root.path().join("src/account_pool.rs"),
        "pub trait Trait {} pub fn thing(){}",
    );
    write(
        root.path().join("src/api_contract_guard.rs"),
        "pub fn check(){}",
    );
    write(root.path().join("src/brand_absence.rs"), body);
    root
}

fn error(body: &str) -> String {
    let root = admitted_tree("pub fn run(){ crate::account_pool::thing(); }");
    write(root.path().join("src/brand_absence.rs"), body);
    refuses(root.path())
}

#[test]
fn active_external_attributes_and_derives_are_unmeasured() {
    for body in [
        "#[evil::inject] pub fn run(){}",
        "use evil::Debug; #[derive(Debug)] pub struct S;",
        "use evil::*; #[derive(Clone)] pub struct S;",
        "#[cfg_attr(unix, evil::inject)] pub fn run(){}",
        "#[evil::inject] use crate::account_pool as pool;",
        "#[evil::inject] macro_rules! local {()=>{}}",
    ] {
        let reason = error(body);
        assert!(reason.contains("cannot be measured"), "{reason}");
    }
}

#[test]
fn definitely_test_only_cfg_attr_is_not_production_uncertainty() {
    let root =
        tree("#[cfg_attr(test, evil::inject)] pub fn run(){ crate::account_pool::thing(); }");
    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/thing"),
        "{graph:?}"
    );
}

#[test]
fn external_macro_use_makes_unqualified_builtin_spelling_ambiguous() {
    let root = tree("#[macro_use] extern crate evil; pub fn run(){ format!(); }");
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\nevil={path=\"evil\"}\n",
    );
    write(
        root.path().join("evil/Cargo.toml"),
        "[package]\nname=\"evil\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    );
    write(
        root.path().join("evil/src/lib.rs"),
        "#[macro_export] macro_rules! format {()=>{ crate::account_pool::thing() }}",
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("an external macro prelude can replace builtin macro spellings");
    assert!(reason.contains("macro format!"), "{reason}");
}

#[test]
fn classifier_uncertainty_removes_exemptions_instead_of_breaking_the_census() {
    let root = tree("use evil::Serialize; #[derive(Serialize)] pub struct S;");
    let shipping = root.path().join("tests/shipping.rs");
    write(&shipping, "pub fn shipping(){}");

    let classifier = anvil::source_scan::paths::TestSourceClassifier::new(root.path())
        .expect("classification has a conservative incomplete mode");
    assert!(
        !classifier.classify(&shipping).unwrap(),
        "unknown expansion must remove a layout-based test exemption"
    );
    let roots = vec![root.path().join("src/lib.rs")];
    assert!(
        anvil::source_scan::paths::declared_production_module_files_from_roots(root.path(), &roots)
            .unwrap()
            .contains(&fs::canonicalize(shipping).unwrap())
    );
}

#[test]
fn audited_default_registry_derives_do_not_make_the_live_graph_unmeasurable() {
    let root = admitted_tree(
        "use serde::Serialize; #[derive(Serialize, serde::Deserialize)]
         pub struct S { #[serde(default = \"default_name\")] name: String }
         fn default_name() -> String { String::new() }
         pub fn run(){ crate::account_pool::thing(); }",
    );
    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/thing"),
        "{graph:?}"
    );
}

#[test]
fn audited_helper_type_bounds_still_contribute_local_dependency_edges() {
    let root = tree(
        "use serde::Serialize; #[derive(Serialize)]
         #[serde(bound(serialize = \"T: crate::account_pool::Trait\"))] pub struct S<T>(T);",
    );
    admitted_profile(root.path());
    write(
        root.path().join("src/account_pool.rs"),
        "pub trait Trait: serde::Serialize {} pub fn thing(){}",
    );
    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/Trait"),
        "{graph:?}"
    );
}

#[test]
fn unsupported_tokio_crate_override_revokes_an_admitted_default_contract() {
    let root = admitted_tree("#[tokio::main] pub async fn run(){ crate::account_pool::thing(); }");
    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/thing"),
        "{graph:?}"
    );
    write(
        root.path().join("src/brand_absence.rs"),
        "#[tokio::main(crate = \"crate::account_pool\")] pub async fn run() {}",
    );
    refuses(root.path());
}

#[test]
fn audited_clap_expression_values_still_contribute_local_dependency_edges() {
    let root = tree(
        "use clap::Parser; #[derive(Parser)]
        #[command(about = crate::account_pool::ABOUT)] pub struct Cli;",
    );
    admitted_profile(root.path());
    write(
        root.path().join("src/account_pool.rs"),
        "pub const ABOUT: &str = \"fixture\"; pub fn thing(){}",
    );
    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/ABOUT"),
        "{graph:?}"
    );
}

#[test]
fn audited_derive_lock_evidence_rejects_tampering_or_an_extra_version() {
    for extra_version in [false, true] {
        let root = admitted_tree("use serde::Serialize; #[derive(Serialize)] pub struct S;");
        let lock = fs::read_to_string(root.path().join("Cargo.lock")).unwrap();
        let changed = if extra_version {
            format!(
                "{lock}\n[[package]]\nname = \"serde\"\nversion = \"9.9.9\"\nsource = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"{}\"\n",
                "a".repeat(64)
            )
        } else {
            lock.replace(
                "4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            )
        };
        assert_ne!(lock, changed, "the lock mutation must have a subject");
        write(root.path().join("Cargo.lock"), &changed);
        refuses(root.path());
    }
}

#[test]
fn lexical_imports_cannot_fall_through_to_an_audited_macro_name() {
    for body in [
        "use serde::Serialize; fn f(){ use unknown::Serialize; #[derive(Serialize)] struct S; }",
        "fn f(){ use unknown as serde; #[derive(serde::Serialize)] struct S; }",
    ] {
        let root = admitted_tree("use serde::Serialize; #[derive(Serialize)] pub struct S;");
        // Unknown identities are inert syntax only, not executable macro packages.
        write(root.path().join("src/brand_absence.rs"), body);
        refuses(root.path());
    }
}

#[test]
fn a_cfg_alternative_package_cannot_inherit_audited_attribute_provenance() {
    let root = admitted_tree("#[tokio::main] pub async fn run() {}");
    let manifest = fs::read_to_string(root.path().join("Cargo.toml")).unwrap();
    // Manifest data only: an unadmitted alternative cannot inherit Tokio's
    // authority. No package is fetched, compiled or redirected at runtime.
    write(
        root.path().join("Cargo.toml"),
        &format!(
            "{manifest}\n[target.'cfg(unix)'.dependencies]\ntokio={{package=\"unadmitted-attributes\",version=\"=1.0.0\"}}\n"
        ),
    );
    refuses(root.path());
}

#[test]
fn exact_registry_macro_provenance_keeps_scanning_caller_tokens() {
    let root = tree(
        "use tracing::info; pub fn run(){
         info!(value = crate::account_pool::thing(), \"measured\"); }",
    );
    admitted_profile(root.path());
    write(
        root.path().join("src/account_pool.rs"),
        "pub trait Trait {} pub fn thing() -> u64 { 1 }",
    );
    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/thing"),
        "{graph:?}"
    );
}

#[test]
fn audited_external_macro_names_require_exact_lock_and_lexical_provenance() {
    for shadowed in [false, true] {
        let root = admitted_tree("use tracing::info; pub fn run(){ info!(\"fixture\"); }");
        if shadowed {
            write(
                root.path().join("src/brand_absence.rs"),
                "use unknown::info; pub fn run(){ info!(); }",
            );
        } else {
            let lock = fs::read_to_string(root.path().join("Cargo.lock")).unwrap();
            let changed = lock.replace(
                "63e71662fa4b2a2c3a26f570f037eb95bb1f85397f3cd8076caed2f026a6d100",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            );
            assert_ne!(lock, changed, "the lock mutation must have a subject");
            write(root.path().join("Cargo.lock"), &changed);
        }
        refuses(root.path());
    }
}

#[test]
fn contributor_cargo_source_authority_revokes_registry_macro_provenance() {
    let root = admitted_tree("use tracing::info; pub fn run(){ info!(\"fixture\"); }");
    write(
        root.path().join(".cargo/config.toml"),
        "[source.crates-io]\nreplace-with=\"vendored\"\n[source.vendored]\ndirectory=\"vendor\"\n",
    );
    refuses(root.path());
}

#[test]
fn local_or_non_registry_crates_cannot_mint_audited_derive_provenance() {
    for dependency in [
        "serde={path=\"serde\"}",
        "serde={git=\"https://invalid.example/serde\"}",
        "serde={version=\"1\",registry=\"private\"}",
    ] {
        let root = tree("use serde::Serialize; #[derive(Serialize)] pub struct S;");
        write(
            root.path().join("Cargo.toml"),
            &format!(
                "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
                 [dependencies]\n{dependency}\n"
            ),
        );
        if dependency.contains("path") {
            write(
                root.path().join("serde/Cargo.toml"),
                "[package]\nname=\"serde\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
            );
            write(
                root.path().join("serde/src/lib.rs"),
                "pub struct Serialize;",
            );
        }
        let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
            .expect_err("untrusted derive implementation must remain unmeasured");
        assert!(reason.contains("derive macro Serialize"), "{reason}");
    }
}
