use std::fs;

use crate::source_scan::paths::production_top_level_modules;

use super::{cargo_metadata, package};

#[test]
fn unsupported_or_empty_member_globs_fail_closed() {
    for members in ["[\"crates/{a,b}\"]", "[\"missing/*\"]"] {
        let root = tempfile::tempdir().expect("workspace");
        fs::write(
            root.path().join("Cargo.toml"),
            format!("[workspace]\nmembers = {members}\n"),
        )
        .expect("workspace manifest");
        assert!(
            production_top_level_modules(root.path()).is_err(),
            "workspace root omission was certified for {members}"
        );
    }
}

#[test]
fn recursive_and_character_class_workspace_globs_match_cargo_members() {
    let root = tempfile::tempdir().expect("workspace");
    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"groups/**/member-[ab]\"]\nexclude=[\"groups/**/member-b\"]\nresolver=\"2\"\n",
    )
    .expect("workspace manifest");
    for (path, name, module) in [
        ("groups/deep/member-a", "member-a", "brand_absence"),
        ("groups/deeper/member-b", "member-b", "excluded_unit"),
    ] {
        let member = root.path().join(path);
        package(&member, name, "", &format!("mod {module};\n"));
        fs::write(
            member.join(format!("src/{module}.rs")),
            "pub fn ships() {}\n",
        )
        .expect("member source");
    }

    assert_eq!(
        production_top_level_modules(root.path()).expect("Cargo-compatible workspace glob"),
        ["brand_absence"].into_iter().map(str::to_owned).collect()
    );
}

#[test]
fn custom_build_script_is_a_contained_production_crate_root() {
    let root = tempfile::tempdir().expect("package");
    fs::create_dir_all(root.path().join("build")).expect("build directory");
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"build-root\"\nversion=\"0.0.0\"\nedition=\"2024\"\nbuild=\"build/entry.rs\"\n",
    )
    .expect("package manifest");
    fs::write(
        root.path().join("build/entry.rs"),
        "mod brand_absence;\nfn main() {}\n",
    )
    .expect("custom build root");
    fs::write(
        root.path().join("build/brand_absence.rs"),
        "pub fn build_logic() {}\n",
    )
    .expect("build module");

    assert_eq!(
        production_top_level_modules(root.path()).expect("custom build-script root"),
        ["brand_absence"].into_iter().map(str::to_owned).collect()
    );
}

#[test]
fn an_explicit_workspace_member_wins_over_the_exclude_glob_like_cargo() {
    let root = tempfile::tempdir().expect("workspace");
    fs::write(
        root.path().join("Cargo.toml"),
        "[workspace]\nmembers=[\"member\"]\nexclude=[\"member\"]\nresolver=\"2\"\n",
    )
    .expect("workspace manifest");
    let member = root.path().join("member");
    package(&member, "explicit-member", "", "mod brand_absence;\n");
    fs::write(member.join("src/brand_absence.rs"), "pub fn ships() {}\n").expect("member source");
    assert_eq!(
        production_top_level_modules(root.path()).expect("explicit member precedence"),
        ["brand_absence"].into_iter().map(str::to_owned).collect()
    );
}

fn class_fixture(pattern: &str, selected: &str, decoy: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("workspace");
    fs::write(
        root.path().join("Cargo.toml"),
        format!("[workspace]\nmembers=[\"crates/{pattern}\"]\nresolver=\"2\"\n"),
    )
    .expect("workspace manifest");
    for (path, name, module) in [
        (selected, "selected-x", "selected_x"),
        (decoy, "decoy", "decoy"),
    ] {
        let member = root.path().join("crates").join(path);
        package(&member, name, "", &format!("mod {module};\n"));
        fs::write(
            member.join(format!("src/{module}.rs")),
            "pub fn ships() {}\n",
        )
        .expect("member source");
    }
    root
}

fn assert_cargo_selects_x(root: &std::path::Path) {
    let metadata = cargo_metadata(root);
    let packages = metadata["packages"]
        .as_array()
        .expect("Cargo packages")
        .iter()
        .map(|package| package["name"].as_str().expect("package name"))
        .collect::<Vec<_>>();
    assert_eq!(packages, ["selected-x"]);
    assert_eq!(
        production_top_level_modules(root).expect("Cargo-compatible character class"),
        ["selected_x"].into_iter().map(str::to_owned).collect()
    );
}

#[test]
fn cargo_character_classes_match_caret_and_first_close_bracket() {
    assert_cargo_selects_x(class_fixture("[^x]", "x", "y").path());
    assert_cargo_selects_x(class_fixture("[!]]", "x", "x]").path());
}

#[test]
fn package_build_true_selects_the_conventional_build_script() {
    let root = tempfile::tempdir().expect("package");
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname=\"build-true\"\nversion=\"0.0.0\"\nedition=\"2024\"\nbuild=true\n",
    )
    .expect("package manifest");
    fs::create_dir_all(root.path().join("src")).expect("source directory");
    fs::write(root.path().join("src/lib.rs"), "pub fn library() {}\n").expect("library root");
    fs::write(
        root.path().join("build.rs"),
        "mod generated_policy; fn main() {}\n",
    )
    .expect("conventional build root");
    fs::write(
        root.path().join("generated_policy.rs"),
        "pub fn shipping_build_logic() {}\n",
    )
    .expect("build helper");

    let metadata = cargo_metadata(root.path());
    let build_roots = metadata["packages"][0]["targets"]
        .as_array()
        .expect("Cargo targets")
        .iter()
        .filter(|target| target["kind"] == serde_json::json!(["custom-build"]))
        .map(|target| {
            fs::canonicalize(target["src_path"].as_str().expect("target path"))
                .expect("canonical Cargo build target")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        build_roots,
        [fs::canonicalize(root.path().join("build.rs")).expect("canonical build script")]
    );
    assert_eq!(
        production_top_level_modules(root.path()).expect("Cargo build=true package"),
        ["generated_policy"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
}
