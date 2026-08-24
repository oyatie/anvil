//! The test-suite gate must still discriminate when the daemon's environment
//! names a shared cargo target directory.
//!
//! `run_cargo_test_gate` builds with `cargo test --no-run` and then runs with
//! `cargo test --no-fail-fast`, and the split is the whole reason the gate can
//! tell "did not compile" (`Errored`, absent evidence) from "tests failed"
//! (`Failed`, an accusation published on a contributor's pull request). The
//! second step only sees what the first built because they share a target
//! directory that belongs to that tree and nothing else.
//!
//! `CARGO_TARGET_DIR` in the inherited environment breaks that -- as does
//! `CARGO_BUILD_TARGET_DIR`, the environment form of cargo's `build.target-dir`
//! and the same hazard under a second name, which is why both are set here and
//! both are dropped from the child. It breaks
//! it back to the pre-fix behaviour rather than to a random failure: `Passed`
//! on a tree whose tests fail, `Failed` on a tree that does not compile -- the
//! two RED lines this lane opened with. It is not a test-only hazard. The
//! daemon certifies many pull requests of the same repository concurrently,
//! each in its own ephemeral worktree, and every one of those worktrees carries
//! the same package name and version and therefore the same artefact path. The
//! gate's own cost note also proposes a shared target directory as the lever to
//! pull if the per-pull-request build becomes unaffordable, so the
//! configuration that silently disarms the gate is one a reader is invited to
//! set.
//!
//! This is a file of its own with exactly one `#[test]` because the fixture is
//! a process-wide environment variable, which is what production would hold.
//! Asserting on the constructed `Command` instead would pin the fix and not the
//! property; this runs the real gate against real crates on disk.

use anvil::queue_healer::{QueueHealer, TestGate};

/// A minimal, dependency-free crate whose `src/lib.rs` is `lib_rs`.
fn crate_with(lib_rs: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"gate-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n\
         [dependencies]\n",
    )
    .expect("write Cargo.toml");
    std::fs::create_dir_all(dir.path().join("src")).expect("mkdir src");
    std::fs::write(dir.path().join("src/lib.rs"), lib_rs).expect("write lib.rs");
    dir
}

#[test]
fn the_gate_discriminates_even_when_the_environment_names_a_shared_target_dir() {
    let shared = tempfile::tempdir().expect("tempdir");
    // SAFETY: this binary contains exactly one `#[test]` and the runtime below
    // is created afterwards, so no other thread of this process is running
    // while the environment is mutated.
    unsafe {
        std::env::set_var("CARGO_TARGET_DIR", shared.path());
        std::env::set_var("CARGO_BUILD_TARGET_DIR", shared.path());
    }

    let rt = tokio::runtime::Runtime::new().expect("runtime");

    let failing = crate_with(
        "pub fn two() -> u32 { 3 }\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
         fn two_is_two() { assert_eq!(super::two(), 2); }\n}\n",
    );
    let passing = crate_with(
        "pub fn two() -> u32 { 2 }\n#[cfg(test)]\nmod tests {\n    #[test]\n    \
         fn two_is_two() { assert_eq!(super::two(), 2); }\n}\n",
    );
    let unbuildable = crate_with("pub fn broken( -> u32 { 2 }\n");

    let red = rt.block_on(QueueHealer::run_local_test_gate(failing.path()));
    assert!(
        matches!(red, TestGate::Failed(_)),
        "a shared target directory must not turn a red suite green; got {red:?}. \
         Two concurrent certifications of one repository share the artefact path, \
         so this is the fleet's normal state, not an edge case."
    );

    let green = rt.block_on(QueueHealer::run_local_test_gate(passing.path()));
    assert!(
        matches!(green, TestGate::Passed(_)),
        "a shared target directory must not accuse a passing suite; got {green:?}"
    );

    let broken = rt.block_on(QueueHealer::run_local_test_gate(unbuildable.path()));
    assert!(
        matches!(broken, TestGate::Errored(_, _)),
        "a tree that does not build ran no test, so it is absent evidence and not \
         a failing suite, whatever the environment says about target directories; \
         got {broken:?}"
    );
}
