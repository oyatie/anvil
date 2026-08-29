//! "Is this file test code?" is answered by Cargo's layout, not by whether the
//! word appears in the path.
//!
//! `whole_file_expansion` exempted a file from the 300-line ceiling when
//! `file_path.contains("test")`. That spared two production files already past
//! the ceiling — `src/attestation_guard.rs`, because "attestation" contains it,
//! and `src/predictive_test_selector/workspace_dag.rs`, because the selector is
//! named after what it selects. Neither is test code; both are exactly what the
//! gate exists to find.
//!
//! `source_scan::paths::is_test_source` is the predicate that already exists,
//! already has its own must-flag and must-spare fixtures, and answers by layout.

use anvil::monorepo_guard::whole_file_expansion::{FileChange, WholeFileExpansion};
use anvil::source_scan::paths::is_test_source;

/// A repository holding one file of `lines` lines at `path`.
fn repo_with(path: &str, lines: usize) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let full = dir.path().join(path);
    std::fs::create_dir_all(full.parent().expect("parent")).expect("mkdir");
    std::fs::write(&full, "pub struct X;\n".repeat(lines)).expect("write");
    dir
}

fn oversized(path: &str) -> Vec<String> {
    let dir = repo_with(path, WholeFileExpansion::MAX_WHOLE_FILE_LINES + 50);
    WholeFileExpansion::evaluate_whole_file(
        dir.path(),
        path,
        &FileChange {
            added: "pub struct X;\n",
            net_lines: 60,
        },
    )
    .into_iter()
    .map(|v| v.category)
    .collect()
}

/// The corpus the substring got wrong, as paths this repository really has.
const PRODUCTION_THAT_SAYS_TEST: &[&str] = &[
    "src/attestation_guard.rs",
    "src/predictive_test_selector/workspace_dag.rs",
    "src/predictive_test_selector/mod.rs",
    "src/supply_chain_guard/slsa_attestation.rs",
    "src/latest_state.rs",
];

/// And the twin: files that genuinely are test code and must stay exempt.
const REALLY_TEST_CODE: &[&str] = &[
    "tests/whatever.rs",
    "crates/x/tests/bar.rs",
    "src/clean_architecture_guard/tests.rs",
    "src/thing_test.rs",
];

#[test]
fn a_production_file_whose_name_says_test_is_not_test_code() {
    for p in PRODUCTION_THAT_SAYS_TEST {
        assert!(
            !is_test_source(p),
            "{p} ships. The substring spelling exempted it from the whole-file \
             ceiling, which is why two files over that ceiling were never \
             reported."
        );
        assert!(
            p.contains("test"),
            "fixture sanity: {p} must contain the substring, or it does not \
             exercise the defect"
        );
    }
}

#[test]
fn test_code_is_still_exempt() {
    for p in REALLY_TEST_CODE {
        assert!(is_test_source(p), "{p} is test code by Cargo's layout");
    }
}

/// The gate itself, not just the predicate: an oversized production file that
/// this change grew is reported, whatever its name says.
#[test]
fn the_ceiling_applies_to_a_grown_production_file_named_after_a_test() {
    let found = oversized("src/attestation_guard.rs");
    assert!(
        found.iter().any(|c| c == "OVERSIZED_WHOLE_FILE"),
        "a 350-line production file this change grew by 60 must be reported; \
         got {found:?}"
    );
}

/// The must-spare twin at the gate level.
#[test]
fn the_ceiling_still_spares_a_grown_test_file() {
    let found = oversized("tests/some_big_suite.rs");
    assert!(
        !found.iter().any(|c| c == "OVERSIZED_WHOLE_FILE"),
        "test code is exempt by layout and must stay exempt; got {found:?}"
    );
}

/// Fix the class, not the instance: nothing else in production may decide
/// test-ness from a path by substring.
///
/// The instance above was one line. The reason it existed is that
/// `path.contains("test")` is the shortest thing to type, `is_test_source` was
/// already imported four lines above it, and nothing objected. This objects.
#[test]
fn no_production_code_decides_test_ness_by_substring() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![root.clone()];

    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().is_none_or(|x| x != "rs") {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&p) else {
                continue;
            };
            let rel = p
                .strip_prefix(env!("CARGO_MANIFEST_DIR"))
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            for (n, line) in body.lines().enumerate() {
                let t = line.trim();
                // Comments quote the defect on purpose; they are not it.
                if t.starts_with("//") {
                    continue;
                }
                // A path variable asked whether it contains the word. The
                // subject matters: `desc.contains("test")` over a task
                // description is a different question with a different right
                // answer, so this is keyed to identifiers that name a path.
                for spelling in [
                    "file_path.contains(\"test\")",
                    "path.contains(\"test\")",
                    "p.contains(\"test\")",
                    "f.contains(\"test\")",
                ] {
                    if t.contains(spelling) {
                        offenders.push(format!("{rel}:{}: {t}", n + 1));
                    }
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "production code decides whether a path is test code by substring:\n{}\n\
         Use `source_scan::paths::is_test_source`, which answers by Cargo's \
         layout and carries its own must-flag and must-spare fixtures. The \
         substring spelling exempted `attestation_guard.rs` and \
         `predictive_test_selector/workspace_dag.rs` from the whole-file \
         ceiling, and both were already past it.",
        offenders.join("\n")
    );
}
