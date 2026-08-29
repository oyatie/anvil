//! Whether a path holds tests rather than shipped code.

/// Rust that exists to test other Rust, by Cargo's layout.
///
/// A `#[cfg(test)]` module inside a production file is deliberately NOT covered:
/// that file ships, and stripping the module is [`super::without_test_modules`].
pub fn is_test_source(path: &str) -> bool {
    path.starts_with("tests/")
        || path.contains("/tests/")
        || path.ends_with("_test.rs")
        || path.ends_with("_tests.rs")
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
        ] {
            assert!(
                !is_test_source(p),
                "{p} ships; a name containing `test` does not make it test code"
            );
        }
    }
}
