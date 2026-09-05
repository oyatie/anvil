use std::fs;

use super::try_is_test_source;

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("fixture repository");
    fs::create_dir_all(root.path().join("src")).expect("source directory");
    fs::create_dir_all(root.path().join("tests")).expect("tests directory");
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname='fixture'\nversion='0.0.0'\nedition='2024'\n",
    )
    .expect("manifest");
    root
}

#[test]
fn production_roles_follow_block_local_path_modules_into_test_layouts() {
    let root = fixture();
    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn f() { #[path = \"../tests/support.rs\"] mod support; support::f(); }\n",
    )
    .expect("crate root");
    fs::write(root.path().join("tests/support.rs"), "pub fn f() {}\n").expect("shipping support");

    assert!(!try_is_test_source(root.path(), &root.path().join("tests/support.rs")).unwrap());
}

#[test]
fn production_roles_override_test_layout_for_arbitrary_module_extensions() {
    let root = fixture();
    fs::write(
        root.path().join("src/lib.rs"),
        "#[path = \"../tests/shipping.inc\"] pub mod shipping;\n",
    )
    .expect("crate root");
    fs::write(
        root.path().join("tests/shipping.inc"),
        "pub fn ships() {}\n",
    )
    .expect("shipping module");

    assert!(
        !try_is_test_source(root.path(), &root.path().join("tests/shipping.inc")).unwrap(),
        "a Rust module's Cargo role must win over its extension and directory spelling"
    );
}

fn assert_production_include(source: &str) {
    let root = fixture();
    fs::write(root.path().join("src/lib.rs"), source).expect("crate root");
    fs::write(root.path().join("tests/shipping.rs"), "pub fn ships() {}\n")
        .expect("included source");
    assert!(!try_is_test_source(root.path(), &root.path().join("tests/shipping.rs")).unwrap());
}

#[test]
fn production_roles_follow_literal_and_aliased_builtin_includes() {
    assert_production_include("include!(\"../tests/shipping.rs\");\n");
    assert_production_include(
        "extern crate std as platform; use platform::include as load; load!(\"../tests/shipping.rs\");\n",
    );
}

#[test]
fn production_roles_follow_reachable_local_macro_includes() {
    assert_production_include(
        "macro_rules! ship { () => { include!(\"../tests/shipping.rs\"); } } ship!();\n",
    );
}

#[test]
fn expression_includes_resolve_local_modules_from_the_included_directory() {
    let root = fixture();
    let generated = root.path().join("tests/generated");
    fs::create_dir_all(&generated).expect("included source directory");
    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn f() { let _ = include!(\"../tests/generated/outer.inc\"); }\n",
    )
    .expect("crate root");
    fs::write(
        generated.join("outer.inc"),
        "{ #[path = \"child.rs\"] mod child; child::f() }\n",
    )
    .expect("included expression");
    fs::write(generated.join("child.rs"), "pub fn f() {}\n").expect("real module");
    fs::write(root.path().join("src/child.rs"), "pub fn decoy() {}\n").expect("caller-dir decoy");

    assert!(
        !try_is_test_source(root.path(), &generated.join("child.rs")).unwrap(),
        "a module compiled through a production expression include is production"
    );
}

#[cfg(unix)]
#[test]
fn source_classification_rejects_symlinks_that_escape_the_subject() {
    use std::os::unix::fs::symlink;

    let root = fixture();
    let outside = tempfile::tempdir().expect("outside directory");
    fs::write(root.path().join("src/lib.rs"), "pub mod leaked;\n").expect("crate root");
    fs::write(
        outside.path().join("leaked.rs"),
        "const SECRET: &str = \"host\";\n",
    )
    .expect("outside source");
    symlink(
        outside.path().join("leaked.rs"),
        root.path().join("src/leaked.rs"),
    )
    .expect("escape symlink");

    for source in [
        root.path().join("src/leaked.rs"),
        root.path().join("tests-leak.inc"),
    ] {
        if source.extension().and_then(|value| value.to_str()) == Some("inc") {
            symlink(outside.path().join("leaked.rs"), &source).expect("non-Rust escape symlink");
        }
        let error = try_is_test_source(root.path(), &source)
            .expect_err("an escaping source must not be read or classified");
        assert!(
            error.contains("outside repository") || error.contains("escapes repository"),
            "{error}"
        );
    }
}

#[test]
fn top_level_tests_rs_is_classified_only_by_its_actual_declaration() {
    for (declaration, expected_test_only) in [
        ("mod tests;\n", false),
        ("#[cfg(test)] pub(crate) mod tests;\n", true),
    ] {
        let root = fixture();
        fs::write(root.path().join("src/lib.rs"), declaration).expect("crate root");
        fs::write(
            root.path().join("src/tests.rs"),
            "const SUBJECT: bool = true;\n",
        )
        .expect("declared source");
        assert_eq!(
            try_is_test_source(root.path(), &root.path().join("src/tests.rs")).unwrap(),
            expected_test_only,
            "declaration {declaration:?}"
        );
    }
}

#[test]
fn custom_cargo_roots_and_autobins_are_authoritative() {
    let root = tempfile::tempdir().expect("source root");
    fs::create_dir_all(root.path().join("code/nested")).expect("custom source directory");
    fs::create_dir_all(root.path().join("src/bin")).expect("conventional bin directory");
    fs::write(
        root.path().join("Cargo.toml"),
        r#"
            [package]
            name = "fixture"
            version = "0.0.0"
            autolib = false
            autobins = false

            [lib]
            path = "code/root.rs"

            [[bin]]
            name = "tool"
            path = "code/tool.rs"
        "#,
    )
    .expect("manifest");
    for (path, source) in [
        (
            "code/root.rs",
            "#[cfg(test)] #[path = \"nested/lib_fixture.rs\"] mod fixture;\n",
        ),
        (
            "code/tool.rs",
            "#[cfg(test)] #[path = \"nested/bin_fixture.rs\"] mod fixture;\n",
        ),
        (
            "code/nested/lib_fixture.rs",
            "const LIB_FIXTURE: bool = true;\n",
        ),
        (
            "code/nested/bin_fixture.rs",
            "const BIN_FIXTURE: bool = true;\n",
        ),
        ("src/bin/ignored.rs", "const NOT_A_TARGET: bool = true;\n"),
        ("src/lib.rs", "#[cfg(test)] mod ignored_lib_fixture;\n"),
        (
            "src/ignored_lib_fixture.rs",
            "const NOT_A_TARGET: bool = true;\n",
        ),
    ] {
        fs::write(root.path().join(path), source).expect("fixture source");
    }

    for path in ["code/nested/lib_fixture.rs", "code/nested/bin_fixture.rs"] {
        assert!(try_is_test_source(root.path(), &root.path().join(path)).unwrap());
    }
    for path in ["src/bin/ignored.rs", "src/ignored_lib_fixture.rs"] {
        assert!(
            !try_is_test_source(root.path(), &root.path().join(path)).unwrap(),
            "disabled automatic target source is conservatively production"
        );
    }
}
