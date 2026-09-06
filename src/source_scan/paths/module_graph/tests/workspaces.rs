use std::fs;
use std::process::Command;

use crate::source_scan::paths::{production_top_level_modules, try_is_test_source};

mod cargo_semantics;

pub(super) fn package(root: &std::path::Path, name: &str, dependencies: &str, source: &str) {
    fs::create_dir_all(root.join("src")).expect("package source directory");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n{dependencies}"
        ),
    )
    .expect("package manifest");
    fs::write(root.join("src/lib.rs"), source).expect("package root");
}

pub(super) fn cargo_metadata(root: &std::path::Path) -> serde_json::Value {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .expect("run Cargo metadata");
    assert!(
        output.status.success(),
        "Cargo rejected fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Cargo metadata JSON")
}

#[test]
fn virtual_workspace_globs_excludes_and_path_members_supply_all_roots() {
    let root = tempfile::tempdir().expect("workspace");
    fs::write(
        root.path().join("Cargo.toml"),
        r#"
            [workspace]
            members = ["members/a*"]
            exclude = ["members/away/"]
            resolver = "2"

            [workspace.dependencies]
            helper = { path = "implicit/helper" }
        "#,
    )
    .expect("virtual manifest");
    fs::create_dir_all(root.path().join("members/away")).expect("excluded grouping directory");

    let alpha = root.path().join("members/alpha");
    package(
        &alpha,
        "alpha",
        r#"
            [dependencies]
            helper = { workspace = true }

            [target.'cfg(unix)'.dependencies]
            direct = { path = "../../implicit/direct" }
        "#,
        r#"
            #[cfg(all(test, unix))]
            mod arbitrary_fixture;
            #[cfg(any(test, unix))]
            mod potentially_shipping;
            mod brand_absence;
        "#,
    );
    fs::write(alpha.join("src/arbitrary_fixture.rs"), "fn fixture() {}\n")
        .expect("test-only member module");
    fs::write(
        alpha.join("src/potentially_shipping.rs"),
        "pub fn ships() {}\n",
    )
    .expect("potential production module");
    fs::write(alpha.join("src/brand_absence.rs"), "pub fn migrate() {}\n").expect("member module");
    fs::create_dir_all(alpha.join("src/bin/utility")).expect("member binary directory");
    fs::write(
        alpha.join("src/bin/utility/main.rs"),
        "#[cfg(test)] mod binary_fixture;\nfn main() {}\n",
    )
    .expect("automatic member binary");
    fs::write(
        alpha.join("src/bin/utility/binary_fixture.rs"),
        "fn fixture() {}\n",
    )
    .expect("automatic binary fixture");

    let helper = root.path().join("implicit/helper");
    package(
        &helper,
        "helper",
        "",
        "#[cfg(test)] mod helper_fixture;\nmod account_pool;\n",
    );
    fs::write(helper.join("src/helper_fixture.rs"), "fn fixture() {}\n")
        .expect("implicit member fixture");
    fs::write(helper.join("src/account_pool.rs"), "pub fn thing() {}\n")
        .expect("implicit member module");

    let direct = root.path().join("implicit/direct");
    package(
        &direct,
        "direct",
        "",
        "#[cfg(test)] mod direct_fixture;\nmod api_contract_guard;\n",
    );
    fs::write(direct.join("src/direct_fixture.rs"), "fn fixture() {}\n")
        .expect("target path member fixture");
    fs::write(
        direct.join("src/api_contract_guard.rs"),
        "pub fn check() {}\n",
    )
    .expect("target path member module");

    for fixture in [
        alpha.join("src/arbitrary_fixture.rs"),
        alpha.join("src/bin/utility/binary_fixture.rs"),
        helper.join("src/helper_fixture.rs"),
        direct.join("src/direct_fixture.rs"),
    ] {
        assert!(
            try_is_test_source(root.path(), &fixture).expect("classify workspace source"),
            "{} was omitted from the workspace declaration graph",
            fixture.display()
        );
    }
    assert!(
        !try_is_test_source(root.path(), &alpha.join("src/potentially_shipping.rs")).unwrap(),
        "cfg(any(test, unix)) can ship and must remain production"
    );
    assert_eq!(
        production_top_level_modules(root.path()).expect("virtual workspace roots"),
        [
            "account_pool",
            "api_contract_guard",
            "brand_absence",
            "potentially_shipping",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
}

#[test]
fn hybrid_workspace_includes_its_root_package_and_members() {
    let root = tempfile::tempdir().expect("workspace");
    fs::create_dir_all(root.path().join("src")).expect("root source");
    fs::write(
        root.path().join("Cargo.toml"),
        r#"
            [package]
            name = "root-package"
            version = "0.0.0"
            edition = "2024"

            [workspace]
            members = ["member"]
            resolver = "2"
        "#,
    )
    .expect("hybrid manifest");
    fs::write(
        root.path().join("src/lib.rs"),
        "#[cfg(test)] mod root_fixture;\nmod root_unit;\n",
    )
    .expect("root package");
    fs::write(root.path().join("src/root_fixture.rs"), "fn fixture() {}\n").expect("root fixture");
    fs::write(root.path().join("src/root_unit.rs"), "pub fn root() {}\n").expect("root module");

    let member = root.path().join("member");
    package(
        &member,
        "member",
        "",
        "#[cfg(test)] mod member_fixture;\nmod member_unit;\n",
    );
    fs::write(member.join("src/member_fixture.rs"), "fn fixture() {}\n").expect("member fixture");
    fs::write(member.join("src/member_unit.rs"), "pub fn member() {}\n").expect("member module");

    assert!(try_is_test_source(root.path(), &root.path().join("src/root_fixture.rs")).unwrap());
    assert!(try_is_test_source(root.path(), &member.join("src/member_fixture.rs")).unwrap());
    assert_eq!(
        production_top_level_modules(root.path()).expect("hybrid roots"),
        ["member_unit", "root_unit"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}
