//! Whether a path holds tests rather than shipped code.

/// Rust that exists to test other Rust, by Cargo's layout.
///
/// A `#[cfg(test)]` module inside a production file is deliberately NOT covered:
/// that file ships, and stripping the module is [`super::without_test_modules`].
///
/// A file named exactly `tests.rs` or `test.rs` IS covered. That is the
/// `#[cfg(test)] mod tests;` split -- the shape a module takes when its
/// fixtures outgrow the file, and the shape
/// `subject_root_escape_hatches_are_named_test` already recognises as test-only.
/// Judged by basename rather than by reading the parent's declaration, because
/// this is a pure path predicate; a `tests.rs` a parent declared WITHOUT
/// `#[cfg(test)]` would be miscounted, and no such file exists in this tree.
pub fn is_test_source(path: &str) -> bool {
    let basename = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/")
        || path.contains("/tests/")
        || path.ends_with("_test.rs")
        || path.ends_with("_tests.rs")
        || basename == "tests.rs"
        || basename == "test.rs"
}

#[cfg(test)]
mod tests {
    use super::is_test_source;

    #[test]
    fn cargos_layout_is_the_rule() {
        for p in [
            "tests/foo.rs",
            "crates/x/tests/bar.rs",
            "src/thing_test.rs",
            "src/thing_tests.rs",
            // The `#[cfg(test)] mod tests;` split.
            "src/clean_architecture_guard/tests.rs",
            "src/thing/test.rs",
        ] {
            assert!(is_test_source(p), "{p} is test code");
        }
    }

    #[test]
    fn a_production_file_that_merely_says_test_is_not_test_code() {
        // The substring spelling this replaces admitted every one of these.
        for p in [
            "src/latest_state.rs",
            "src/test_harness_runner.rs",
            "src/contest/mod.rs",
            "src/attestation_guard.rs",
            "src/protest.rs",
            // A basename that merely ENDS in the word is not the split.
            "src/latests.rs",
            "src/contests.rs",
        ] {
            assert!(
                !is_test_source(p),
                "{p} ships; a name containing `test` does not make it test code"
            );
        }
    }
}
