use std::fs;

use crate::source_scan::paths::try_is_test_source;

#[test]
fn explicit_production_roots_override_test_directory_names() {
    let root = tempfile::tempdir().expect("source root");
    let tests = root.path().join("tests");
    fs::create_dir_all(&tests).expect("custom source directory");
    fs::write(
        root.path().join("Cargo.toml"),
        r#"
            [package]
            name = "fixture"
            version = "0.0.0"
            autolib = false
            autobins = false

            [[bin]]
            name = "shipping"
            path = "tests/shipping.rs"
        "#,
    )
    .expect("manifest");
    fs::write(tests.join("shipping.rs"), "mod helper; fn main() {}\n").expect("binary root");
    fs::write(tests.join("helper.rs"), "pub fn shipping() {}\n").expect("binary helper");

    assert!(!try_is_test_source(root.path(), &tests.join("shipping.rs")).unwrap());
    assert!(!try_is_test_source(root.path(), &tests.join("helper.rs")).unwrap());
}
