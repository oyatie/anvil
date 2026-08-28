//! Whether a path holds tests rather than shipped code.
//!
//! One answer, because there were four and they disagreed. Two were
//! byte-identical copies of this predicate; one walked path components and
//! admitted more suffixes; the fourth was `path.contains("test")`, which spares
//! any production file whose name happens to carry the word and is the spelling
//! that let `#[cfg(test)]` fixtures be charged to the modules holding them.
//!
//! This lives beside the strippers rather than inside `mod.rs` because that
//! file sits at the 300-line budget, and a scoping question is not a reason to
//! breach it.

/// Rust that exists to test other Rust.
///
/// Cargo's own layout is the whole rule: an integration test is a file under
/// `tests/`, and a unit test lives beside its subject under a `_test` or
/// `_tests` suffix by convention. Anything else is shipped code, whatever its
/// name suggests.
///
/// A `#[cfg(test)]` module inside a production file is NOT covered here and
/// must not be: the file ships, and stripping the module is
/// [`super::without_test_modules`]'s job. Conflating the two questions is what
/// made a module answerable for its own fixtures.
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
