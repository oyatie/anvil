//! Cargo build-closure cases that are easy to omit from custom discovery.

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

fn legacy(root: &Path) {
    write(
        root.join("legacy/Cargo.toml"),
        "[package]\nname = \"legacy-package\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nname = \"legacy_surface\"\n",
    );
    write(
        root.join("legacy/src/lib.rs"),
        "pub mod account_pool { pub fn thing() {} }\n",
    );
}

fn versioned_override(root: &Path, directory: &str, package: &str, version: &str, lib: &str) {
    write(
        root.join(directory).join("Cargo.toml"),
        &format!(
            "[package]\nname=\"{package}\"\nversion=\"{version}\"\nedition=\"2024\"\n\
             [lib]\nname=\"{lib}\"\n"
        ),
    );
    write(
        root.join(directory).join("src/lib.rs"),
        "pub mod account_pool { pub fn thing() {} }",
    );
}

fn app(root: &Path, dependencies: &str, body: &str) {
    write(
        root.join("app/Cargo.toml"),
        &format!(
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{dependencies}\n"
        ),
    );
    write(
        root.join("app/src/lib.rs"),
        "mod account_pool; mod api_contract_guard; mod brand_absence;\n",
    );
    write(root.join("app/src/account_pool.rs"), "pub fn thing() {}\n");
    write(
        root.join("app/src/api_contract_guard.rs"),
        "pub fn check() {}\n",
    );
    write(root.join("app/src/brand_absence.rs"), body);
}

fn assert_account_pool_edge(root: &Path) {
    let graph = anvil::source_scan::paths::production_module_dependencies(root).unwrap();
    let measured = graph.get("brand_absence").unwrap();
    assert!(measured.contains("account_pool/thing"), "{graph:?}");
}

fn cargo_check(root: &Path) {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["check", "--offline", "--all-targets"])
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

#[test]
fn workspace_exclude_does_not_remove_an_explicit_path_dependency() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"app\"]\nexclude=[\"legacy\"]\nresolver=\"2\"\n",
    );
    app(
        root.path(),
        "[dependencies]\nlegacy-package={path=\"../legacy\"}",
        "pub fn run(){ legacy_surface::account_pool::thing(); }",
    );
    legacy(root.path());
    cargo_check(root.path());
    assert_account_pool_edge(root.path());
}

#[test]
fn target_specific_workspace_dependency_keeps_its_effective_alias() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"app\",\"legacy\"]\nresolver=\"2\"\n[workspace.dependencies]\ndep-alias={path=\"legacy\",package=\"legacy-package\"}\n",
    );
    app(
        root.path(),
        "[target.'cfg(target_arch = \"wasm32\")'.dependencies]\ndep-alias={workspace=true}",
        "#[cfg(target_arch = \"wasm32\")] pub fn run(){ dep_alias::account_pool::thing(); }",
    );
    legacy(root.path());
    cargo_check(root.path());
    assert_account_pool_edge(root.path());
}

#[test]
fn renamed_version_dependency_resolves_through_a_local_patch() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"app\"]\nresolver=\"2\"\n[patch.crates-io]\nlegacy-package={path=\"legacy\"}\n",
    );
    app(
        root.path(),
        "[dependencies]\nerr={package=\"legacy-package\",version=\"0.1\"}",
        "pub fn run(){ err::account_pool::thing(); }",
    );
    legacy(root.path());
    cargo_check(root.path());
    assert_account_pool_edge(root.path());
}

#[test]
fn renamed_version_dependency_resolves_through_a_local_replace() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"app\"]\nresolver=\"2\"\n[replace]\n\"legacy-package:0.1.0\"={path=\"legacy\"}\n",
    );
    app(
        root.path(),
        "[dependencies]\nerr={package=\"legacy-package\",version=\"=0.1.0\"}",
        "pub fn run(){ err::account_pool::thing(); }",
    );
    legacy(root.path());
    // Source/manifest closure contract only: this invented registry identity is
    // not in the offline cache. This does not claim Cargo compiled the fixture.
    assert_account_pool_edge(root.path());
}

#[test]
fn every_same_package_patch_candidate_contributes_its_possible_library_alias() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"app\"]\nresolver=\"2\"\n\
         [patch.crates-io]\nfoo_v1={package=\"foo\",path=\"foo-v1\"}\n\
         foo_v2={package=\"foo\",path=\"foo-v2\"}\n",
    );
    app(
        root.path(),
        "[dependencies]\nfoo=\"=2.0.0\"",
        "pub fn run(){ right_surface::account_pool::thing(); }",
    );
    versioned_override(root.path(), "foo-v1", "foo", "1.0.0", "wrong_surface");
    versioned_override(root.path(), "foo-v2", "foo", "2.0.0", "right_surface");
    cargo_check(root.path());
    assert_account_pool_edge(root.path());
}

#[test]
fn every_same_package_replace_candidate_contributes_its_possible_library_alias() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"app\"]\nresolver=\"2\"\n\
         [replace]\n\"itoa:1.0.17\"={path=\"itoa-old\"}\n\
         \"itoa:1.0.18\"={path=\"itoa-new\"}\n",
    );
    app(
        root.path(),
        "[dependencies]\nitoa=\"=1.0.18\"",
        "pub fn run(){ right_surface::account_pool::thing(); }",
    );
    versioned_override(root.path(), "itoa-old", "itoa", "1.0.17", "wrong_surface");
    versioned_override(root.path(), "itoa-new", "itoa", "1.0.18", "right_surface");
    cargo_check(root.path());
    assert_account_pool_edge(root.path());
}

#[test]
fn one_physical_root_is_measured_in_each_cargo_target_context() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"app\"\nversion=\"0.1.0\"\nedition=\"2024\"\nbuild=\"shared.rs\"\n[lib]\npath=\"shared.rs\"\n[build-dependencies]\ndep-alias={path=\"legacy\",package=\"legacy-package\"}\n",
    );
    write(
        root.path().join("shared.rs"),
        "mod account_pool; mod api_contract_guard; mod brand_absence; fn main(){}",
    );
    write(root.path().join("account_pool.rs"), "pub fn thing(){}\n");
    write(
        root.path().join("api_contract_guard.rs"),
        "pub fn check(){}\n",
    );
    write(
        root.path().join("brand_absence.rs"),
        "pub fn run(){ dep_alias::account_pool::thing(); }\n",
    );
    legacy(root.path());
    assert_account_pool_edge(root.path());
}

#[test]
fn an_explicit_dev_path_package_is_still_part_of_the_measured_cargo_closure() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"app\"]\nexclude=[\"helper\"]\nresolver=\"2\"\n",
    );
    app(
        root.path(),
        "[dev-dependencies]\nhelper={path=\"../helper\"}",
        "pub fn run(){}",
    );
    write(
        root.path().join("helper/Cargo.toml"),
        "[package]\nname=\"helper\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    );
    write(
        root.path().join("helper/src/lib.rs"),
        "mod account_pool; mod api_contract_guard; mod brand_absence;",
    );
    write(
        root.path().join("helper/src/account_pool.rs"),
        "pub fn thing(){}",
    );
    write(
        root.path().join("helper/src/api_contract_guard.rs"),
        "pub fn check(){}",
    );
    write(
        root.path().join("helper/src/brand_absence.rs"),
        "pub fn run(){ crate::account_pool::thing(); }",
    );
    cargo_check(root.path());
    assert_account_pool_edge(root.path());
}
