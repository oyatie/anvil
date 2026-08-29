//! D-8's crate boundary holds for a crate at the repository root.
//!
//! A module directory inside a crate is governed by the compiler and by
//! D-30/D-35, not by D-8, and a Rust module is loaded by `mod name;` rather
//! than by the path literal the predicate looks for. Directories outside the
//! crate's source tree stay governed: `docs/` is the witness for that half.

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
