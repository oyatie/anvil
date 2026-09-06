use std::fs;

use super::source_tree;
use crate::source_scan::paths::{try_is_test_source, try_module_source};

#[test]
fn a_path_overridden_module_is_addressable_by_its_reported_file_path() {
    let root = tempfile::tempdir().expect("source root");
    let src = source_tree(root.path());
    fs::write(
        src.join("widget.rs"),
        "mod parent;\nconst WIDGET: bool = true;\n",
    )
    .expect("widget root");
    fs::write(
        src.join("widget/parent.rs"),
        "#[path = \"renamed.rs\"]\nmod logical_child;\n",
    )
    .expect("declaring parent");
    fs::write(
        src.join("widget/renamed.rs"),
        "const REPORTED_PATH: bool = true;\n",
    )
    .expect("path-overridden child");

    let source = try_module_source("src/widget/renamed", root.path())
        .expect("declared path-overridden source");
    assert!(source.contains("REPORTED_PATH"));
}

#[test]
fn a_path_override_inside_an_inline_module_uses_rustcs_nested_context() {
    let root = tempfile::tempdir().expect("source root");
    let src = source_tree(root.path());
    fs::write(
        src.join("widget.rs"),
        "mod outer { #[path = \"renamed.rs\"] mod child; }\n",
    )
    .expect("inline declaring module");
    fs::create_dir_all(src.join("widget/outer")).expect("rustc inline-module directory");
    fs::write(
        src.join("widget/outer/renamed.rs"),
        "const RUSTC_PATH: bool = true;\n",
    )
    .expect("nested path override");
    fs::create_dir_all(src.join("outer")).expect("old incorrect directory");
    fs::write(
        src.join("outer/renamed.rs"),
        "const WRONG_PATH: bool = true;\n",
    )
    .expect("wrong-path sentinel");

    let source = try_module_source("src/widget", root.path()).expect("production module");
    assert!(source.contains("RUSTC_PATH"));
    assert!(!source.contains("WRONG_PATH"));
}

#[test]
fn a_nested_path_override_with_parent_components_keeps_one_file_identity() {
    let root = tempfile::tempdir().expect("source root");
    let src = source_tree(root.path());
    fs::write(
        src.join("widget.rs"),
        "mod outer { #[cfg(test)] #[path = \"../fixture.rs\"] mod child; }\n",
    )
    .expect("inline declaring module");
    fs::write(
        src.join("widget/fixture.rs"),
        "const TEST_ONLY: bool = true;\n",
    )
    .expect("normalized path override");

    assert!(try_is_test_source(root.path(), &src.join("widget/fixture.rs")).unwrap());
    let source = try_module_source("src/widget", root.path()).expect("production module");
    assert!(!source.contains("TEST_ONLY"));
}

#[test]
fn ambiguous_file_and_directory_module_forms_are_refused_like_rustc() {
    let root = tempfile::tempdir().expect("source root");
    let src = root.path().join("src");
    fs::create_dir_all(src.join("widget")).expect("module directory");
    fs::write(src.join("lib.rs"), "mod widget;\n").expect("crate root");
    fs::write(src.join("widget.rs"), "const FILE: bool = true;\n").expect("file form");
    fs::write(src.join("widget/mod.rs"), "const DIRECTORY: bool = true;\n")
        .expect("directory form");

    let error = try_module_source("src/widget", root.path())
        .expect_err("rustc rejects E0761 rather than merging both module bodies");
    assert!(error.contains("both file and directory sources"), "{error}");
}

#[test]
fn a_path_overridden_nonroot_main_file_keeps_its_stem_module_directory() {
    let root = tempfile::tempdir().expect("source root");
    let src = root.path().join("src");
    fs::create_dir_all(src.join("nested/main")).expect("redirected module directory");
    fs::write(
        src.join("lib.rs"),
        "#[path = \"nested/main.rs\"] mod redirected;\n",
    )
    .expect("crate root");
    fs::write(
        src.join("nested/main.rs"),
        "#[cfg(test)] mod arbitrary_fixture;\npub const SHIPS: bool = true;\n",
    )
    .expect("redirected nonroot main.rs");
    fs::write(
        src.join("nested/main/arbitrary_fixture.rs"),
        "const FIXTURE: bool = true;\n",
    )
    .expect("nested fixture");

    assert!(
        try_is_test_source(root.path(), &src.join("nested/main/arbitrary_fixture.rs")).unwrap()
    );
    let source =
        try_module_source("src/redirected", root.path()).expect("redirected production module");
    assert!(source.contains("SHIPS"));
    assert!(!source.contains("FIXTURE"));
}

#[test]
fn cfg_attr_path_selects_the_non_test_source_instead_of_a_conventional_decoy() {
    let root = tempfile::tempdir().expect("source root");
    let src = source_tree(root.path());
    fs::write(
        src.join("widget.rs"),
        "#[cfg_attr(not(test), path = \"shipping.rs\")] mod selected;\n",
    )
    .expect("conditional path declaration");
    fs::write(
        src.join("widget/selected.rs"),
        "const CONVENTIONAL_DECOY: bool = true;\n",
    )
    .expect("conventional decoy");
    fs::write(
        src.join("shipping.rs"),
        "const SHIPPING_SOURCE: bool = true;\n",
    )
    .expect("shipping source");

    let source = try_module_source("src/widget", root.path()).expect("production module");
    assert!(source.contains("SHIPPING_SOURCE"));
    assert!(!source.contains("CONVENTIONAL_DECOY"));
}

#[test]
fn target_conditional_path_variants_are_all_measured_conservatively() {
    let root = tempfile::tempdir().expect("source root");
    let src = source_tree(root.path());
    fs::write(
        src.join("widget.rs"),
        "#[cfg_attr(unix, path = \"unix.rs\")] mod selected;\n",
    )
    .expect("conditional path declaration");
    fs::write(
        src.join("widget/selected.rs"),
        "const OTHER_TARGET: bool = true;\n",
    )
    .expect("conventional variant");
    fs::write(src.join("unix.rs"), "const UNIX_TARGET: bool = true;\n").expect("unix variant");

    let source = try_module_source("src/widget", root.path()).expect("production variants");
    assert!(source.contains("OTHER_TARGET"));
    assert!(source.contains("UNIX_TARGET"));
}

#[test]
fn path_overrides_cannot_escape_the_repository() {
    let root = tempfile::tempdir().expect("source root");
    let outside = tempfile::NamedTempFile::new().expect("outside source");
    fs::write(outside.path(), "const OUTSIDE: bool = true;\n").expect("outside source body");
    let src = source_tree(root.path());
    fs::write(
        src.join("widget.rs"),
        format!("#[path = {:?}] mod escaped;\n", outside.path()),
    )
    .expect("escaping declaration");

    let error = try_module_source("src/widget", root.path())
        .expect_err("outside source cannot enter the measured corpus");
    assert!(error.contains("escapes repository"), "{error}");
}
