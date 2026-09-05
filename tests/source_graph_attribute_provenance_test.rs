//! Syntax-transforming attributes and macro-prelude uncertainty fail closed.

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

fn cargo_check(root: &Path) {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["check", "--offline"])
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .output()
        .expect("run Cargo fixture");
    assert!(
        output.status.success(),
        "fixture must compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn evil_proc_macro(root: &Path) {
    write(
        root.join("evil/Cargo.toml"),
        "[package]\nname=\"evil\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [lib]\nproc-macro=true\n",
    );
    write(
        root.join("evil/src/lib.rs"),
        "extern crate proc_macro; use proc_macro::TokenStream; \
         #[proc_macro_derive(Serialize)] pub fn serialize(_:TokenStream)->TokenStream { \
             TokenStream::new() } \
         #[proc_macro_derive(Debug)] pub fn debug(_:TokenStream)->TokenStream { \
             TokenStream::new() } \
         #[proc_macro_derive(Clone)] pub fn clone(_:TokenStream)->TokenStream { \
             TokenStream::new() } \
         #[proc_macro_attribute] pub fn inject(_:TokenStream,item:TokenStream)->TokenStream { \
             item } \
         #[proc_macro] pub fn info(_:TokenStream)->TokenStream { \"()\".parse().unwrap() }",
    );
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
    let root = tree(body);
    evil_proc_macro(root.path());
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\nevil={path=\"evil\"}\n",
    );
    cargo_check(root.path());
    anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("unproved syntax expansion must prevent measurement")
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
    let root = tree(
        "use serde::Serialize; #[derive(Serialize, serde::Deserialize)] \
         pub struct S { #[serde(default = \"default_name\")] name: String } \
         fn default_name() -> String { String::new() } \
         pub fn run(){ crate::account_pool::thing(); }",
    );
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\nserde={version=\"=1.0.229\",features=[\"derive\"]}\n",
    );
    cargo_check(root.path());

    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/thing"),
        "{graph:?}"
    );
}

#[test]
fn audited_helper_type_bounds_still_contribute_local_dependency_edges() {
    let root = tree(
        "use serde::Serialize; #[derive(Serialize)] \
         #[serde(bound(serialize = \"T: crate::account_pool::Trait\"))] pub struct S<T>(T);",
    );
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\nserde={version=\"=1.0.229\",features=[\"derive\"]}\n",
    );
    write(
        root.path().join("src/account_pool.rs"),
        "pub trait Trait: serde::Serialize {} pub fn thing(){}",
    );
    cargo_check(root.path());

    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/Trait"),
        "{graph:?}"
    );
}

#[test]
fn audited_tokio_crate_override_still_contributes_local_dependency_edges() {
    let root = tree("#[tokio::main(crate = \"crate::account_pool\")] pub async fn run() {}");
    write(
        root.path().join("src/account_pool.rs"),
        "pub use tokio::*; pub fn thing(){}",
    );
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\ntokio={version=\"=1.53.1\",features=[\"macros\",\"rt-multi-thread\"]}\n",
    );
    cargo_check(root.path());

    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"]
            .iter()
            .any(|edge| edge.starts_with("account_pool")),
        "{graph:?}"
    );
}

#[test]
fn audited_clap_expression_values_still_contribute_local_dependency_edges() {
    let root = tree(
        "use clap::Parser; #[derive(Parser)] \
         #[command(about = crate::account_pool::ABOUT)] pub struct Cli;",
    );
    write(
        root.path().join("src/account_pool.rs"),
        "pub const ABOUT: &str = \"fixture\"; pub fn thing(){}",
    );
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\nclap={version=\"=4.6.6\",features=[\"derive\"]}\n\
         clap_derive={version=\"=4.6.4\"}\n",
    );
    cargo_check(root.path());

    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/ABOUT"),
        "{graph:?}"
    );
}

#[test]
fn audited_derive_lock_evidence_rejects_tampering_or_an_extra_version() {
    for extra in [
        r#"[[package]]
name = "serde"
version = "9.9.9"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa""#,
        "",
    ] {
        let root = tree("use serde::Serialize; #[derive(Serialize)] pub struct S;");
        write(
            root.path().join("Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
             [dependencies]\nserde={version=\"=1.0.229\",features=[\"derive\"]}\n",
        );
        cargo_check(root.path());
        if extra.is_empty() {
            let mut lock = fs::read_to_string(root.path().join("Cargo.lock")).unwrap();
            lock = lock.replace(
                "4148590afebada386688f18773da617792bf2ef03ffc1e4cbd2b1d45b023e0ba",
                "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            );
            write(root.path().join("Cargo.lock"), &lock);
        } else {
            let mut lock = fs::read_to_string(root.path().join("Cargo.lock")).unwrap();
            lock.push_str(extra);
            write(root.path().join("Cargo.lock"), &lock);
        }
        let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
            .expect_err("changed lock evidence must revoke proc-macro provenance");
        assert!(reason.contains("derive macro Serialize"), "{reason}");
    }
}

#[test]
fn lexical_imports_cannot_fall_through_to_an_audited_macro_name() {
    for body in [
        "use serde::Serialize; fn f(){ use evil::Serialize; #[derive(Serialize)] struct S; }",
        "fn f(){ use evil as serde; #[derive(serde::Serialize)] struct S; }",
    ] {
        let root = tree(body);
        write(
            root.path().join("Cargo.toml"),
            "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
             [dependencies]\nserde={version=\"=1.0.229\",features=[\"derive\"]}\nevil={path=\"evil\"}\n",
        );
        evil_proc_macro(root.path());
        cargo_check(root.path());
        let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
            .expect_err("lexical macro ambiguity must not inherit audited provenance");
        assert!(reason.contains("derive macro"), "{reason}");
    }
}

#[test]
fn a_cfg_alternative_package_cannot_inherit_audited_attribute_provenance() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("src/lib.rs"),
        "extern crate self as async_std; pub mod task { pub fn block_on<F>(_: F) {} } \
         mod account_pool; mod api_contract_guard; mod brand_absence;",
    );
    write(root.path().join("src/account_pool.rs"), "pub fn thing(){}");
    write(
        root.path().join("src/api_contract_guard.rs"),
        "pub fn check(){}",
    );
    write(
        root.path().join("src/brand_absence.rs"),
        "#[tokio::main] pub async fn main() {}",
    );
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [target.'cfg(unix)'.dependencies]\n\
         tokio={package=\"async-attributes\",version=\"=1.1.2\"}\n\
         [target.'cfg(windows)'.dependencies]\n\
         tokio={version=\"=1.53.1\",features=[\"macros\",\"rt\"]}\n",
    );
    cargo_check(root.path());

    let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("all cfg-possible bindings must share audited attribute provenance");
    assert!(reason.contains("attribute"), "{reason}");
}

#[test]
fn exact_registry_macro_provenance_keeps_scanning_caller_tokens() {
    let root = tree(
        "use tracing::info; pub fn run(){ \
         info!(value = crate::account_pool::thing(), \"measured\"); }",
    );
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\ntracing=\"=0.1.44\"\n",
    );
    cargo_check(root.path());

    let graph = anvil::source_scan::paths::production_module_dependencies(root.path()).unwrap();
    assert!(
        graph["brand_absence"].contains("account_pool/thing"),
        "{graph:?}"
    );
}

#[test]
fn audited_external_macro_names_require_exact_lock_and_lexical_provenance() {
    for shadowed in [false, true] {
        let body = if shadowed {
            "use evil::info; pub fn run(){ info!(); }"
        } else {
            "use tracing::info; pub fn run(){ info!(\"fixture\"); }"
        };
        let root = tree(body);
        write(
            root.path().join("Cargo.toml"),
            &format!(
                "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
                 [dependencies]\ntracing=\"=0.1.44\"\n{}",
                if shadowed {
                    "evil={path=\"evil\"}\n"
                } else {
                    ""
                }
            ),
        );
        if shadowed {
            evil_proc_macro(root.path());
        }
        cargo_check(root.path());
        if !shadowed {
            let mut lock = fs::read_to_string(root.path().join("Cargo.lock")).unwrap();
            lock = lock.replace(
                "63e71662fa4b2a2c3a26f570f037eb95bb1f85397f3cd8076caed2f026a6d100",
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            );
            write(root.path().join("Cargo.lock"), &lock);
        }
        let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
            .expect_err("macro provenance must be exact and unshadowed");
        assert!(reason.contains("macro info!"), "{reason}");
    }
}

#[test]
fn contributor_cargo_source_authority_revokes_registry_macro_provenance() {
    let root = tree("use tracing::info; pub fn run(){ info!(\"fixture\"); }");
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\n\
         [dependencies]\ntracing=\"=0.1.44\"\n",
    );
    cargo_check(root.path());
    write(
        root.path().join(".cargo/config.toml"),
        "[source.crates-io]\nreplace-with=\"vendored\"\n[source.vendored]\ndirectory=\"vendor\"\n",
    );
    let reason = anvil::source_scan::paths::production_module_dependencies(root.path())
        .expect_err("contributor source replacement invalidates registry macro evidence");
    assert!(reason.contains("macro info!"), "{reason}");
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
