//! Which repository is Anvil's own is a property of the build, not of the
//! process environment.
//!
//! `tests/docguard_oracle_repair_test.rs` hardcodes `oyatie/anvil` and, until
//! now, left the reason as an open question. Under this project's method the
//! tests are the specification, so leaving it open meant the implementer picked
//! — and one legal pick is `Config::self_repo`, which reads `SELF_REPO`
//! (`src/config.rs:126`) through `Config::from_env`, which calls
//! `dotenvy::dotenv()` (`src/config.rs:76`) and MUTATES the process
//! environment. That pick has three separate defects:
//!
//!   1. In a parallel test binary, mutating `environ` while other threads read
//!      it is a data race.
//!   2. It silently reparents ownership to whatever `.env` happens to sit in
//!      the working directory, so the whole of the issue-#27 suite passes on a
//!      clean machine and flakes on a developer's.
//!   3. In production a mis-set `SELF_REPO` makes the sync rewrite and push a
//!      watched repository's docs — which is issue #27 verbatim, arrived at by
//!      a different route.
//!
//! So it is settled here rather than deferred: ownership does not read the
//! environment. This case is the only test in its own binary precisely because
//! it mutates the environment to prove that nothing reads it; putting it in the
//! main suite would create the very race it exists to forbid.

use anvil::doc_guard::corpus_sync::sync_published_counts;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use std::path::Path;
use tempfile::tempdir;

const ANVIL: &str = "oyatie/anvil";

/// The slug this test misdirects `SELF_REPO` at. It is one of the repositories
/// Anvil actually reviews, so getting this wrong in production is the live
/// harm, not a hypothetical one.
const IMPERSONATED: &str = "oyatie/console";

const OWNED_PAGES: &[&str] = &[
    "README.md",
    "docs/doctrine.md",
    "openapi/openapi.yaml",
    "docs/adr/0001-console.md",
    "docs/decisions/0001-console.md",
];

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

/// Mirrors `drifting_page()` in the main suite: a page of Anvil's whose
/// published claims are deliberately not `TOTAL_GATES`.
fn drifting_page() -> String {
    format!(
        "# Anvil\n\
         \n\
         The fabric ships behind a {}-gate release check.\n\
         It replaced the sixty-gate pilot programme.\n",
        TOTAL_GATES + 1
    )
}

/// Mirrors `watched_repo_page()` in the main suite: a page belonging to a
/// repository that is not Anvil's, carrying all three mutations `rewrite_page`
/// performs.
fn watched_repo_page() -> String {
    format!(
        "# Console\n\
         \n\
         The console ships behind a {}-gate release check.\n\
         It replaced the sixty-gate pilot programme.\n\
         Roadmap. The console does **not** yet amend existing documents such as `README.md`. Support is planned.\n\
         \n\
         None of this is Anvil's corpus.\n",
        TOTAL_GATES + 1
    )
}

#[test]
fn anvil_ownership_is_a_compile_time_constant_not_a_process_environment_variable() {
    // SAFETY: this binary contains exactly one test, so no other thread of this
    // process is running while the environment is mutated. That is also the
    // reason the case lives here instead of in the main suite.
    unsafe {
        std::env::set_var("SELF_REPO", IMPERSONATED);
    }

    // The environment now says Anvil is `oyatie/console`. It is not.
    let console_dir = tempdir().unwrap();
    let page = watched_repo_page();
    for owned in OWNED_PAGES {
        write(&console_dir.path().join(owned), &page);
    }

    let console = sync_published_counts(IMPERSONATED, console_dir.path(), TOTAL_GATES).unwrap();

    for owned in OWNED_PAGES {
        assert_eq!(
            std::fs::read_to_string(console_dir.path().join(owned)).unwrap(),
            page,
            "SELF_REPO={IMPERSONATED} must not make {IMPERSONATED}'s {owned} Anvil's \
             to rewrite. A predicate built on `Config::self_repo` hands a watched \
             repository's published docs to a rewrite that then gets committed and \
             pushed onto the contributor's branch — issue #27, reached through the \
             environment instead of through the argument"
        );
    }
    assert!(
        console.rewritten.is_empty(),
        "nothing in {IMPERSONATED} may be reported as rewritten: {:?}",
        console.rewritten
    );
    assert!(
        console.remaining_drift.is_empty(),
        "{IMPERSONATED}'s counts are not drift against Anvil's TOTAL_GATES: {:?}",
        console.remaining_drift
    );
    let reason = console.not_applicable.as_deref().unwrap_or_else(|| {
        panic!("{IMPERSONATED} is still not Anvil's, so the skip must still be stated")
    });
    assert!(
        !reason.trim().is_empty(),
        "the stated reason must actually say something"
    );

    // And the converse: the environment cannot disown Anvil's own repository
    // either. A `Config::self_repo` predicate fails here too — it would now
    // decline to repair Anvil's own published counts, silently removing gate
    // 1's corpus enforcement from every Anvil pull request.
    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), &drifting_page());

    let anvil = sync_published_counts(ANVIL, anvil_dir.path(), TOTAL_GATES).unwrap();
    let got = std::fs::read_to_string(anvil_dir.path().join("README.md")).unwrap();

    assert_eq!(
        anvil.rewritten,
        vec!["README.md".to_string()],
        "SELF_REPO={IMPERSONATED} must not disown oyatie/anvil"
    );
    assert!(
        got.contains(&format!("{TOTAL_GATES}-gate")),
        "Anvil's own README must still be rewritten to TOTAL_GATES: {got}"
    );
    assert!(
        !got.contains("sixty-gate"),
        "Anvil's own spelled-out claim must still be repaired: {got}"
    );
    assert!(
        anvil.not_applicable.is_none(),
        "the sync applied to Anvil, so it must not report otherwise: {:?}",
        anvil.not_applicable
    );
    assert!(
        anvil.remaining_drift.is_empty(),
        "every claim on this page is repairable: {:?}",
        anvil.remaining_drift
    );
}
