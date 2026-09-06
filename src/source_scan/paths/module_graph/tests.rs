use super::{production_top_level_modules, try_is_test_source, try_module_source};
use std::fs;
use std::path::{Path, PathBuf};

mod path_overrides;
mod role_classification;
mod target_layout;
mod workspaces;

pub(super) fn source_tree(root: &Path) -> PathBuf {
    let src = root.join("src");
    fs::create_dir_all(src.join("widget")).expect("module directory");
    fs::write(src.join("lib.rs"), "mod widget;\n").expect("crate root");
    src
}

#[test]
fn reads_file_and_directory_forms_that_are_actually_declared() {
    let root = tempfile::tempdir().expect("source root");
    let src = source_tree(root.path());
    fs::write(
        src.join("widget.rs"),
        "mod support;\nmod nested;\nconst ROOT_FORM: &str = \"root\";\n",
    )
    .expect("root form");
    fs::write(
        src.join("widget/support.rs"),
        "const FILE_CHILD: &str = \"file child\";\n",
    )
    .expect("file child");
    fs::create_dir_all(src.join("widget/nested")).expect("directory child");
    fs::write(
        src.join("widget/nested/mod.rs"),
        "const DIRECTORY_CHILD: &str = \"directory child\";\n",
    )
    .expect("directory child");

    let source = try_module_source("src/widget", root.path()).expect("production module");
    assert!(source.contains("ROOT_FORM"));
    assert!(source.contains("FILE_CHILD"));
    assert!(source.contains("DIRECTORY_CHILD"));
}

#[test]
fn unconditional_tests_ship_and_exact_cfg_test_fixtures_do_not() {
    let root = tempfile::tempdir().expect("source root");
    let src = source_tree(root.path());
    fs::write(
        src.join("widget.rs"),
        "mod tests;\n#[cfg(test)]\nmod fixtures;\nconst AFTER_DECLS: bool = true;\n",
    )
    .expect("root form");
    fs::write(
        src.join("widget/tests.rs"),
        "const UNCONDITIONAL_TESTS_SHIP: bool = true;\n",
    )
    .expect("production child");
    fs::write(
        src.join("widget/fixtures.rs"),
        "const CFG_TEST_FIXTURE: bool = true;\n",
    )
    .expect("test child");

    let source = try_module_source("src/widget", root.path()).expect("production module");
    assert!(source.contains("UNCONDITIONAL_TESTS_SHIP"));
    assert!(source.contains("AFTER_DECLS"));
    assert!(!source.contains("CFG_TEST_FIXTURE"));
    assert!(!try_is_test_source(root.path(), &src.join("widget/tests.rs")).unwrap());
    assert!(try_is_test_source(root.path(), &src.join("widget/fixtures.rs")).unwrap());
}

#[test]
fn a_top_level_tests_basename_cannot_override_its_cfg_declaration() {
    let root = tempfile::tempdir().expect("source root");
    let src = root.path().join("src");
    fs::create_dir_all(&src).expect("source directory");
    fs::write(src.join("lib.rs"), "#[cfg(test)]\nmod tests;\n").expect("crate root");
    fs::write(src.join("tests.rs"), "const ONLY_A_FIXTURE: bool = true;\n").expect("test module");

    assert!(try_is_test_source(root.path(), &src.join("tests.rs")).unwrap());
    assert!(try_module_source("src/tests", root.path()).is_err());
    assert!(production_top_level_modules(root.path()).is_err());
}

#[test]
fn an_explicit_mod_subject_reads_its_root_without_unrelated_children() {
    let root = tempfile::tempdir().expect("source root");
    let src = root.path().join("src");
    fs::create_dir_all(src.join("widget")).expect("module directory");
    fs::write(src.join("lib.rs"), "mod widget;\n").expect("crate root");
    fs::write(
        src.join("widget/mod.rs"),
        "mod child;\nconst ROOT_ONLY: bool = true;\n",
    )
    .expect("module root");
    fs::write(
        src.join("widget/child.rs"),
        "const CHILD_SOURCE: bool = true;\n",
    )
    .expect("module child");

    let root_only = try_module_source("src/widget/mod", root.path()).expect("module root");
    assert!(root_only.contains("ROOT_ONLY"));
    assert!(!root_only.contains("CHILD_SOURCE"));
    assert!(
        try_module_source("src/widget", root.path())
            .expect("whole module")
            .contains("CHILD_SOURCE")
    );
}

#[test]
fn cargo_binary_roots_and_their_declared_children_are_source_subjects() {
    let root = tempfile::tempdir().expect("source root");
    let binary = root.path().join("src/bin/tool");
    fs::create_dir_all(&binary).expect("binary directory");
    fs::write(
        binary.join("main.rs"),
        "mod input;\nconst ROOT: bool = true;\n",
    )
    .expect("binary root");
    fs::write(binary.join("input.rs"), "const INPUT: bool = true;\n").expect("binary child");

    let main = try_module_source("src/bin/tool/main", root.path()).expect("Cargo binary root");
    assert!(main.contains("ROOT"));
    assert!(!main.contains("INPUT"));
    assert!(
        try_module_source("src/bin/tool", root.path())
            .expect("whole Cargo binary")
            .contains("INPUT")
    );
    assert!(
        try_module_source("src/bin/tool/input", root.path())
            .expect("declared binary child")
            .contains("INPUT")
    );
}

#[test]
fn nested_inline_and_binary_declarations_decide_test_only_files() {
    let root = tempfile::tempdir().expect("source root");
    let src = root.path().join("src");
    fs::create_dir_all(src.join("widget/outer")).expect("nested module directory");
    fs::create_dir_all(src.join("bin/tool")).expect("binary module directory");
    fs::write(src.join("lib.rs"), "mod widget;\n").expect("library root");
    fs::write(
        src.join("widget.rs"),
        r#"
            mod outer {
                #[cfg( test )]
                #[path = "fixture.rs"]
                pub(crate) mod conditional;
                #[path = "shipping_test.rs"]
                mod unconditional;
            }
        "#,
    )
    .expect("nested declarations");
    fs::write(
        src.join("widget/outer/fixture.rs"),
        "const FIXTURE: bool = true;\n",
    )
    .expect("test-only child");
    fs::write(
        src.join("widget/outer/shipping_test.rs"),
        "const SHIPPING: bool = true;\n",
    )
    .expect("production child");
    fs::write(
        src.join("bin/tool.rs"),
        "#[cfg(test)] #[path = \"tool/fixture.rs\"] mod fixture; fn main() {}\n",
    )
    .expect("binary root");
    fs::write(
        src.join("bin/tool/fixture.rs"),
        "const BIN_FIXTURE: bool = true;\n",
    )
    .expect("binary test child");

    assert!(try_is_test_source(root.path(), &src.join("widget/outer/fixture.rs")).unwrap());
    assert!(!try_is_test_source(root.path(), &src.join("widget/outer/shipping_test.rs")).unwrap());
    assert!(try_is_test_source(root.path(), &src.join("bin/tool/fixture.rs")).unwrap());
}

#[test]
fn absence_of_a_crate_root_is_not_evidence_that_a_source_is_test_only() {
    let root = tempfile::tempdir().expect("source root");
    let source = root.path().join("orphan_tests.rs");
    fs::write(
        &source,
        "const PRODUCTION_UNTIL_PROVED_OTHERWISE: bool = true;\n",
    )
    .expect("orphan source");

    assert!(!try_is_test_source(root.path(), &source).expect("conservative classification"));
}
