//! The certification oracle must not lie about, or vandalise, documentation.
//!
//! Issue #27: the corpus sync rewrites the gate counts of whatever repository
//! it is pointed at, using Anvil's private `TOTAL_GATES`, and the pipeline then
//! commits and pushes the result onto the contributor's branch.
//!
//! Issue #28: the exemption-marker deletion in `rewrite_page` takes its start
//! from the previous sentence boundary but its end from end-of-line, so it
//! destroys any prose that follows the marker on the same line and fuses the
//! surviving prefix with the next line.
//!
//! Issue #29 is pinned by the in-module suite in `src/doc_guard/mod.rs`,
//! because the branch it concerns is only reachable after the `agy` doc-parity
//! probe has run and no test may spawn a model.

use anvil::doc_guard::DocGuard;
use anvil::doc_guard::corpus_sync::sync_published_counts;
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Anvil's own repository: the only one whose published gate counts are
/// `TOTAL_GATES`.
const ANVIL: &str = "oyatie/anvil";

/// Two of the repositories Anvil reviews. `TOTAL_GATES` is meaningless in both.
const WATCHED: &[&str] = &["oyatie/oyatie", "oyatie/console"];

/// A page that publishes a gate count the sync would want to rewrite.
const DRIFTING_README: &str = "# Console\n\nThe console ships behind a 12-gate release check.\n";

/// The real pre-#12 README table, verbatim (`git show 508a66e^:README.md`).
/// The exemption marker sits mid-line, inside the DocGuard row's cell — the
/// only layout these markers were ever written for.
const HISTORICAL_README_TABLE: &str = "| Quality Gate | Description |\n\
|---|---|\n\
| **📚 Documentation & ADR Parity** | Verifies public APIs and platform doctrine, and creates missing ADRs (`DocGuard`). Note: it does **not** yet amend existing documents such as `README.md` or `CHANGELOG.md` — see the roadmap. |\n\
| **🛡️ Cedar Policy & IAM Boundaries** | Verifies AWS Cedar authorization policy coverage & tenant bounds (`CedarGuard`) |\n";

const DOCGUARD_ROW_PREFIX: &str = "| **📚 Documentation & ADR Parity** |";
const CEDAR_ROW: &str = "| **🛡️ Cedar Policy & IAM Boundaries** | Verifies AWS Cedar authorization policy coverage & tenant bounds (`CedarGuard`) |";
const EXEMPTION_MARKER: &str = "does **not** yet amend existing documents";

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

// =========================================================================
// Issue #27 — the corpus sync is scoped to Anvil's own repository
// =========================================================================

#[test]
fn the_corpus_sync_rewrites_anvils_own_published_counts_but_not_a_watched_repositorys() {
    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), DRIFTING_README);

    let anvil = sync_published_counts(ANVIL, anvil_dir.path(), TOTAL_GATES).unwrap();
    let anvil_readme = std::fs::read_to_string(anvil_dir.path().join("README.md")).unwrap();

    assert_eq!(
        anvil.rewritten,
        vec!["README.md".to_string()],
        "Anvil's own published counts are still the sync's business"
    );
    assert!(
        anvil_readme.contains(&format!("{TOTAL_GATES}-gate")),
        "Anvil's README must be rewritten to TOTAL_GATES: {anvil_readme}"
    );

    for repo in WATCHED {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), DRIFTING_README);
        write(&dir.path().join("docs/doctrine.md"), DRIFTING_README);
        write(
            &dir.path().join("docs/adr/0001-console.md"),
            DRIFTING_README,
        );

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in ["README.md", "docs/doctrine.md", "docs/adr/0001-console.md"] {
            let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
            assert_eq!(
                got, DRIFTING_README,
                "{repo}: {owned} is not Anvil's page and TOTAL_GATES says nothing \
                 about it, so the sync must leave it byte-identical"
            );
        }
        assert!(
            sync.rewritten.is_empty(),
            "{repo}: nothing in another repository may be reported as rewritten: {:?}",
            sync.rewritten
        );
        assert!(
            sync.remaining_drift.is_empty(),
            "{repo}: another repository's gate counts are not drift against \
             Anvil's TOTAL_GATES and must not fail its PR: {:?}",
            sync.remaining_drift
        );
    }
}

#[test]
fn a_corpus_sync_that_did_not_apply_says_so_instead_of_passing_silently() {
    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), DRIFTING_README);
    let anvil = sync_published_counts(ANVIL, anvil_dir.path(), TOTAL_GATES).unwrap();
    assert!(
        anvil.not_applicable.is_none(),
        "the sync did apply to Anvil, so it must not report otherwise: {:?}",
        anvil.not_applicable
    );

    for repo in WATCHED {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), DRIFTING_README);

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        let reason = sync.not_applicable.as_deref().unwrap_or_else(|| {
            panic!(
                "{repo}: a skipped sync that reports an empty rewrite list and no \
                 drift is indistinguishable from a clean page; the skip must be stated"
            )
        });
        assert!(
            !reason.trim().is_empty(),
            "{repo}: the stated reason must actually say something"
        );
    }
}

#[test]
fn reviewing_a_repository_that_is_not_anvil_leaves_its_owned_pages_byte_identical() {
    // Deliberately short-circuits on a frontmatter violation, which
    // `ensure_documentation_parity` checks after the corpus sync and before the
    // `agy` doc-parity probe. That keeps the test off the probe path entirely.
    // The claim under test is about the bytes on disk, so it holds whichever
    // branch the guard returns through.
    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), DRIFTING_README);
    write(&dir.path().join("docs/doctrine.md"), DRIFTING_README);
    write(
        &dir.path().join("tenancy/policy.md"),
        "---\nstatus: active\ncanonical_authority: true\n---\n\n# Tenancy\n",
    );

    let diff_ctx = PrDiffContext {
        repo: "oyatie/console".to_string(),
        pr_number: 77,
        base_branch: "main".to_string(),
        base_sha: "base-sha".to_string(),
        head_sha: "head-sha".to_string(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: "diff --git a/tenancy/policy.md b/tenancy/policy.md\n+claim\n".to_string(),
        changed_files: vec!["tenancy/policy.md".to_string()],
        repo_working_dir: PathBuf::from("."),
    };

    let report = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            DocGuard::new("low".to_string())
                .ensure_documentation_parity(
                    "oyatie/console",
                    dir.path(),
                    &diff_ctx,
                    "feat: tenancy policy",
                    "",
                )
                .await
                .unwrap()
        });

    for owned in ["README.md", "docs/doctrine.md"] {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(owned)).unwrap(),
            DRIFTING_README,
            "reviewing oyatie/console must not edit that repository's {owned}; \
             the edit would be committed and pushed onto the contributor's branch"
        );
    }
    assert!(
        !report
            .files_created_or_updated
            .iter()
            .any(|f| f == "README.md" || f == "docs/doctrine.md"),
        "no page of another repository may be reported as touched: {:?}",
        report.files_created_or_updated
    );
}

// =========================================================================
// Issue #28 — removing the exemption removes one sentence, not one line
// =========================================================================

#[test]
fn an_exemption_marker_inside_a_table_row_leaves_the_row_and_its_neighbour_intact() {
    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), HISTORICAL_README_TABLE);

    let sync = sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).unwrap();
    let got = std::fs::read_to_string(dir.path().join("README.md")).unwrap();

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption sentence is the thing being removed: {got}"
    );
    assert_eq!(
        got.lines().count(),
        HISTORICAL_README_TABLE.lines().count(),
        "the table must keep every row it started with:\n{got}"
    );

    let docguard_row = got
        .lines()
        .find(|l| l.starts_with(DOCGUARD_ROW_PREFIX))
        .unwrap_or_else(|| panic!("the DocGuard row must survive:\n{got}"));
    assert!(
        docguard_row.ends_with('|'),
        "the DocGuard row must keep its closing pipe and stay a table row: {docguard_row}"
    );
    assert!(
        docguard_row.contains(
            "Verifies public APIs and platform doctrine, and creates missing ADRs (`DocGuard`)."
        ),
        "the prose before the exemption sentence must survive: {docguard_row}"
    );
    assert!(
        got.lines().any(|l| l == CEDAR_ROW),
        "the following row must not be fused into the one above it:\n{got}"
    );
    assert!(
        sync.remaining_drift.is_empty(),
        "{:?}",
        sync.remaining_drift
    );
}

#[test]
fn prose_following_the_exemption_sentence_on_the_same_line_survives() {
    let dir = tempdir().unwrap();
    write(
        &dir.path().join("README.md"),
        "# Anvil\n\nAlpha sentence. DocGuard does **not** yet amend existing documents. Beta sentence.\nGamma line.\n",
    );

    sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).unwrap();
    let got = std::fs::read_to_string(dir.path().join("README.md")).unwrap();

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption sentence is the thing being removed: {got}"
    );
    assert!(
        got.contains("Alpha sentence."),
        "the sentence before the exemption must survive: {got}"
    );
    assert!(
        got.contains("Beta sentence."),
        "the sentence after the exemption, on the same line, must survive: {got}"
    );

    let surviving = got
        .lines()
        .find(|l| l.contains("Alpha sentence."))
        .unwrap_or_else(|| panic!("{got}"));
    assert!(
        surviving.contains("Beta sentence."),
        "removing a sentence must not split the line it sat on: {surviving}"
    );
    assert!(
        got.lines().any(|l| l == "Gamma line."),
        "the next line must stay a line of its own, not be fused upward: {got}"
    );
    assert_eq!(
        got.lines().count(),
        4,
        "line structure must be preserved:\n{got}"
    );
}
