//! D-8's boundary must hold for a crate at the repository root.
//!
//! The predicate stops at "the nearest enclosing build target": above it D-8
//! decides, below it the crate does. That boundary was derived by splitting a
//! manifest path on `/`, which yields nothing for a root-level `Cargo.toml` --
//! so a single-crate repository registered no crate root at all, excluded
//! nothing, and judged every module directory under `src/` as if it were a
//! capability directory.
//!
//! Every such directory came back `Orphan`, because a Rust module is loaded by
//! `mod name;` and not by a path literal. On anvil that was 66 refusals
//! including `src/github/`, `src/fixer/` and `src/local_inner_loop/` -- all
//! declared in `src/lib.rs`. A conversion tool acting on that list would
//! delete the repository.
//!
//! oyatie never exhibited it: all 28,996 of its manifests are nested, so the
//! split always succeeded. The defect was reachable only on the repository
//! shape every managed repo starts in.

use anvil::shape::facade::admit::{AdmitRequest, admit};

#[tokio::test]
async fn a_module_directory_of_a_root_level_crate_is_not_governed_by_d8() {
    let req = AdmitRequest {
        repo_dir: std::env::current_dir().unwrap(),
        rev: "HEAD".to_string(),
    };
    let report = admit(&req).await.expect("admit over anvil's own tree");

    let refused: Vec<&str> = report.refused().iter().map(|v| v.dir.as_str()).collect();

    // Each of these is declared in src/lib.rs. None is a capability directory.
    for live in [
        "src/github/",
        "src/fixer/",
        "src/local_inner_loop/",
        "src/metrics/",
        "src/dashboard/",
        "src/monorepo_guard/",
    ] {
        assert!(
            !refused.contains(&live),
            "{live} is a Rust module declared in src/lib.rs, not a directory D-8 governs; \
             refusing it means the crate boundary was not found. Refused set: {refused:?}"
        );
    }

    // The boundary must not swallow the repository root either: directories
    // outside the crate's source tree are exactly what D-8 exists to judge.
    let governed: Vec<&str> = report.verdicts.iter().map(|v| v.dir.as_str()).collect();
    assert!(
        governed.contains(&"docs/"),
        "docs/ sits outside the crate source tree and must stay governed; got {governed:?}"
    );
}
