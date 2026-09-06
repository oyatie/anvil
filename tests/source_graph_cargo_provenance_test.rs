//! Cargo namespace provenance for the production dependency graph.

use std::collections::BTreeSet;
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

fn workspace(dependency: &str, body: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"app\", \"legacy\"]\nresolver = \"2\"\n",
    );
    write(
        root.path().join("app/Cargo.toml"),
        &format!(
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependency}\n"
        ),
    );
    write(
        root.path().join("app/src/lib.rs"),
        "mod account_pool; mod api_contract_guard; mod brand_absence;\n",
    );
    write(
        root.path().join("app/src/account_pool.rs"),
        "pub fn thing() {}\n",
    );
    write(
        root.path().join("app/src/api_contract_guard.rs"),
        "pub fn check() {}\n",
    );
    write(root.path().join("app/src/brand_absence.rs"), body);
    write(
        root.path().join("legacy/Cargo.toml"),
        "[package]\nname = \"legacy-package\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nname = \"legacy_surface\"\n",
    );
    write(
        root.path().join("legacy/src/lib.rs"),
        "pub mod account_pool { pub fn thing() {} }\n",
    );
    root
}

fn dependencies(root: &Path) -> BTreeSet<String> {
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
        .expect("run Cargo fixture");
    assert!(
        output.status.success(),
        "fixture must compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn renamed_path_dependency_is_visible_inside_an_external_module() {
    let root = workspace(
        "dep-alias = { path = \"../legacy\", package = \"legacy-package\" }",
        "pub fn run() { dep_alias::account_pool::thing(); }\n",
    );
    cargo_check(root.path());
    assert!(dependencies(root.path()).contains("account_pool/thing"));
}

#[test]
fn an_explicit_rename_wins_even_when_normalized_spellings_match() {
    let root = workspace(
        "legacy_package = { path = \"../legacy\", package = \"legacy-package\" }",
        "pub fn run() { legacy_package::account_pool::thing(); }\n",
    );
    cargo_check(root.path());
    assert!(dependencies(root.path()).contains("account_pool/thing"));
}

#[test]
fn unrenamed_path_dependency_uses_its_custom_library_name() {
    let root = workspace(
        "legacy-package = { path = \"../legacy\" }",
        "pub fn run() { legacy_surface::account_pool::thing(); }\n",
    );
    cargo_check(root.path());
    assert!(dependencies(root.path()).contains("account_pool/thing"));
}

#[test]
fn a_local_module_definitely_shadows_a_manifest_crate_alias() {
    let root = workspace(
        "dep-alias = { path = \"../legacy\", package = \"legacy-package\" }",
        "mod dep_alias { pub fn safe() {} } pub fn run() { dep_alias::safe(); }\n",
    );
    write(
        root.path().join("app/src/lib.rs"),
        "mod account_pool; mod api_contract_guard; mod brand_absence;\n",
    );
    cargo_check(root.path());
    let measured = dependencies(root.path());
    // The nested alias is inside this subject; self edges are intentionally omitted.
    assert!(measured.is_empty(), "{measured:?}");
    assert!(!measured.contains("account_pool/thing"), "{measured:?}");
}

#[test]
fn a_binary_root_can_resolve_its_package_library_name() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\nname = \"custom_lib\"\n",
    );
    write(
        root.path().join("src/lib.rs"),
        "pub mod account_pool { pub fn thing() {} }\nmod api_contract_guard;\n",
    );
    write(
        root.path().join("src/api_contract_guard.rs"),
        "pub fn check() {}\n",
    );
    write(
        root.path().join("src/main.rs"),
        "mod brand_absence; fn main() { brand_absence::run(); }\n",
    );
    write(
        root.path().join("src/brand_absence.rs"),
        "pub fn run() { custom_lib::account_pool::thing(); }\n",
    );
    cargo_check(root.path());
    assert!(dependencies(root.path()).contains("account_pool/thing"));
}
