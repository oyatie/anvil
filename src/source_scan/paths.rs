//! Whether a path holds tests rather than shipped code.
use std::fs;
use std::path::Path;

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

/// Production Rust for a module, whether it is a file or a directory.
///
/// `module_source("src/merge_enlister", root)` reads `merge_enlister.rs` if
/// that is what exists, and every `.rs` under `merge_enlister/` if it is a
/// directory instead. Test modules are stripped, so callers get what ships.
///
/// # Why this is a function and not a path each caller writes
///
/// Thirty-four test files read a specific `src/<name>.rs` by path, three
/// hundred and thirty-two times between them. Splitting a file into a directory
/// is routine here -- the oversized-file ratchet demands it, and this tree has
/// done it to `gate_proof`, `pre_merge_guard`, `harness::rules`, `occupancy`
/// and `merge_enlister` -- and every one of those reads goes BLIND that day.
/// Blind, not failing: a scan that reads nothing finds nothing wrong.
///
/// # Panics
///
/// When no source exists for `module`. A scan that cannot find its subject must
/// say so rather than report nothing wrong with it.
pub fn module_source(module: &str, repo_root: &Path) -> String {
    let base = repo_root.join(module);
    let mut paths = Vec::new();
    let as_file = base.with_extension("rs");
    if as_file.is_file() {
        paths.push(as_file);
    }
    if base.is_dir() {
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    paths.push(p);
                }
            }
        }
    }
    assert!(
        !paths.is_empty(),
        "no production source for `{module}` under {}: a scan that cannot find \
         its subject reports nothing wrong with it",
        repo_root.display()
    );
    paths.sort();
    paths
        .iter()
        .map(|p| super::without_test_modules(&fs::read_to_string(p).unwrap_or_default()))
        .collect::<Vec<_>>()
        .join("\n")
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
