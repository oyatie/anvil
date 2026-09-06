use std::path::Path;

use anyhow::Result;

use super::AddedLines;

/// Splits the added lines into the coverable source files and a count of added
/// test-file lines.
///
/// Added test lines are counted and reported, never divided by: a PR that adds
/// a thousand lines of test file and one unexecuted production line is at 0%,
/// which is the whole point.
pub(super) fn partition_added_lines(
    added: &AddedLines,
    repo_dir: &Path,
) -> Result<(AddedLines, usize)> {
    let mut coverable = AddedLines::new();
    let mut test_lines = 0usize;
    let mut rust_test_sources = None;
    for (path, lines) in added {
        if lines.is_empty() {
            continue;
        }
        let is_rust = path
            .rsplit_once('.')
            .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("rs"));
        let is_test = if is_rust {
            if !repo_dir.join(path).is_file() {
                false
            } else {
                if rust_test_sources.is_none() {
                    rust_test_sources = Some(
                        crate::source_scan::paths::TestSourceClassifier::new(repo_dir)
                            .map_err(anyhow::Error::msg)?,
                    );
                }
                rust_test_sources
                    .as_ref()
                    .expect("initialized Rust test-source classifier")
                    .classify(Path::new(path))
                    .map_err(anyhow::Error::msg)?
            }
        } else {
            is_non_rust_test_path(path)
        };
        if is_test {
            test_lines += lines.len();
        } else if is_coverable_source(path) {
            coverable.insert(path.clone(), lines.clone());
        }
    }
    Ok((coverable, test_lines))
}

/// Whether a coverage tool could plausibly report on this file.
///
/// Extension-based on purpose: a Markdown or TOML change has no executable
/// lines, so demanding coverage of it is a false red, and a gate that cannot be
/// satisfied gets bypassed.
fn is_coverable_source(path: &str) -> bool {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    matches!(
        ext.as_str(),
        "rs" | "ts"
            | "tsx"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "go"
            | "py"
            | "c"
            | "cc"
            | "cpp"
            | "cxx"
            | "h"
            | "hh"
            | "hpp"
            | "java"
            | "kt"
            | "rb"
            | "cs"
            | "swift"
            | "scala"
    ) && path.contains('.')
}

/// Whether a non-Rust path is test code.
///
/// Rust is classified separately from Cargo layout plus its parsed declaration
/// graph. Other languages retain their ecosystem filename conventions. These
/// are matched on whole components/names, never as a bare substring.
fn is_non_rust_test_path(path: &str) -> bool {
    for part in path.split('/') {
        if matches!(
            part,
            "tests" | "test" | "__tests__" | "spec" | "specs" | "testdata"
        ) {
            return true;
        }
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.split('.').next().unwrap_or(name);
    stem.starts_with("test_")
        || stem.ends_with("_test")
        || stem.ends_with("_tests")
        || stem.ends_with("_spec")
        || name.contains(".test.")
        || name.contains(".spec.")
}
