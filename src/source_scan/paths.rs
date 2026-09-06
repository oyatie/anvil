//! Whether a path holds tests rather than shipped code.

mod module_graph;

pub use module_graph::{
    TestSourceClassifier, declared_production_module_files_from_roots, declared_test_module_files,
    declared_test_module_files_from_roots, module_source, production_module_dependencies,
    production_top_level_modules, try_is_test_source, try_module_source,
};

/// Rust that exists to test other Rust, by Cargo's layout.
///
/// A `#[cfg(test)]` module inside a production file is deliberately NOT covered:
/// that file ships, and stripping the module is [`super::without_test_modules`].
///
/// External modules are deliberately NOT classified by basename. `mod tests;`
/// ships while `#[cfg(test)] mod fixtures;` does not; only the declaration can
/// tell them apart. Call [`try_is_test_source`] when a source tree is
/// available.
pub fn is_test_source(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    let mut below_src = false;
    for component in normalized.split('/') {
        if component == "src" {
            below_src = true;
        } else if component == "tests" {
            return !below_src;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::is_test_source;

    #[test]
    fn cargos_layout_is_the_rule() {
        for p in ["tests/foo.rs", "crates/x/tests/bar.rs"] {
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
            // A declaration, not this basename, decides whether these ship.
            "src/tests.rs",
            "src/clean_architecture_guard/tests.rs",
            "src/thing/test.rs",
            "src\\thing\\tests.rs",
            "src/thing_test.rs",
            "src/thing_tests.rs",
            "src\\thing_tests.rs",
            "src/widget/tests/helper.rs",
        ] {
            assert!(
                !is_test_source(p),
                "{p} ships; a name containing `test` does not make it test code"
            );
        }
    }
}
