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
//! Issue #29's live path sits behind the `agy` doc-parity probe. It is pinned
//! here through `DocGuard::with_probe_override`, and in the in-module suite in
//! `src/doc_guard/mod.rs`. No test in either place may spawn a model.

use anvil::doc_guard::corpus_sync::sync_published_counts;
use anvil::doc_guard::{DocGuard, DocParityEvaluation};
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Anvil's own repository: the only one whose published gate counts are
/// `TOTAL_GATES`.
const ANVIL: &str = "oyatie/anvil";

/// The same repository, in the case a GitHub event might deliver it in.
/// See `anvil_is_recognised_case_insensitively_and_only_as_a_whole_slug`.
const ANVIL_SLUGS: &[&str] = &["oyatie/anvil", "Oyatie/Anvil"];

/// Two of the repositories Anvil reviews. `TOTAL_GATES` is meaningless in both.
const WATCHED: &[&str] = &["oyatie/oyatie", "oyatie/console"];

/// Slugs that are near misses for Anvil's own. Every one of them belongs to
/// somebody else, so the sync must treat them exactly as it treats
/// `oyatie/console`. A substring, prefix, suffix, or owner-blind predicate
/// separates `oyatie/anvil` from `oyatie/console` and still corrupts these.
const NEAR_MISSES: &[&str] = &[
    "oyatie/anvil-sdk",
    "oyatie/anvildocs",
    "attacker/anvil",
    // No owner at all: not a slug Anvil can recognise as its own.
    "anvil",
    // The boundary: nothing is known about the repository under review.
    "",
];

/// Every page `collect_owned_pages` claims, so a partial skip is caught too.
const OWNED_PAGES: &[&str] = &[
    "README.md",
    "docs/doctrine.md",
    "openapi/openapi.yaml",
    "docs/adr/0001-console.md",
    "docs/decisions/0001-console.md",
];

const EXEMPTION_MARKER: &str = "does **not** yet amend existing documents";
const PLAIN_EXEMPTION_MARKER: &str = "does not yet amend existing documents";

/// A page publishing a gate count that is deliberately *not* `TOTAL_GATES`, so
/// the fixture stays a drift fixture whatever `TOTAL_GATES` becomes.
fn drifting_page() -> String {
    format!(
        "# Console\n\nThe console ships behind a {}-gate release check.\n",
        TOTAL_GATES + 1
    )
}

/// The real pre-#12 README table, verbatim (`git show 508a66e^:README.md`).
/// The exemption marker sits mid-line, inside the DocGuard row's cell — the
/// only layout these markers were ever written for.
const HISTORICAL_README_TABLE: &str = "| Quality Gate | Description |\n\
|---|---|\n\
| **📚 Documentation & ADR Parity** | Verifies public APIs and platform doctrine, and creates missing ADRs (`DocGuard`). Note: it does **not** yet amend existing documents such as `README.md` or `CHANGELOG.md` — see the roadmap. |\n\
| **🛡️ Cedar Policy & IAM Boundaries** | Verifies AWS Cedar authorization policy coverage & tenant bounds (`CedarGuard`) |\n";

const DOCGUARD_ROW_PREFIX: &str = "| **📚 Documentation & ADR Parity** |";

/// The DocGuard row with the exemption *sentence* gone — not merely the marker
/// phrase. `Note: it` opens that sentence and `— see the roadmap.` closes it;
/// both belong to it and both go.
const DOCGUARD_ROW_AFTER: &str = "| **📚 Documentation & ADR Parity** | Verifies public APIs and platform doctrine, and creates missing ADRs (`DocGuard`). |";

const CEDAR_ROW: &str = "| **🛡️ Cedar Policy & IAM Boundaries** | Verifies AWS Cedar authorization policy coverage & tenant bounds (`CedarGuard`) |";

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

/// Collapses runs of whitespace inside each line to a single space and trims
/// the line.
///
/// Assertions compare normalised text so they pin *which words survive* without
/// dictating whether the implementation leaves one space or two where a
/// sentence used to be. Line count and line boundaries are asserted separately
/// and unnormalised, because those are load-bearing.
fn normalise(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

/// Runs the sync over a single Anvil-owned `README.md` and returns the bytes
/// left on disk together with the reported drift.
fn rewrite_anvil_readme(body: &str) -> (String, Vec<String>) {
    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), body);
    let sync = sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).unwrap();
    let got = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
    (got, sync.remaining_drift)
}

fn diff_ctx(repo: &str, changed: &[&str]) -> PrDiffContext {
    PrDiffContext {
        repo: repo.to_string(),
        pr_number: 77,
        base_branch: "main".to_string(),
        base_sha: "base-sha".to_string(),
        head_sha: "head-sha".to_string(),
        is_incremental: false,
        previous_head_sha: None,
        diff_content: "diff --git a/src/lib.rs b/src/lib.rs\n+pub fn newly_public() {}\n"
            .to_string(),
        changed_files: changed.iter().map(|f| (*f).to_string()).collect(),
        repo_working_dir: PathBuf::from("."),
    }
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

// =========================================================================
// Issue #27 — the corpus sync is scoped to Anvil's own repository
// =========================================================================

#[test]
fn the_corpus_sync_rewrites_anvils_own_published_counts_but_not_a_watched_repositorys() {
    let drifting = drifting_page();

    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), &drifting);

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
    assert!(
        anvil.not_applicable.is_none(),
        "the sync did apply to Anvil, so it must not report otherwise: {:?}",
        anvil.not_applicable
    );

    for repo in WATCHED {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &drifting);
        }

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in OWNED_PAGES {
            let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
            assert_eq!(
                got, drifting,
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
fn a_slug_that_merely_resembles_anvils_is_still_somebody_elses_repository() {
    let drifting = drifting_page();

    for repo in NEAR_MISSES {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &drifting);
        }

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in OWNED_PAGES {
            let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
            assert_eq!(
                got, drifting,
                "{repo:?} is not oyatie/anvil; {owned} must be byte-identical. A \
                 predicate that only separates oyatie/anvil from oyatie/console \
                 still rewrites and pushes this repository's docs"
            );
        }
        assert!(
            sync.rewritten.is_empty(),
            "{repo:?}: nothing may be reported as rewritten: {:?}",
            sync.rewritten
        );
        assert!(
            sync.remaining_drift.is_empty(),
            "{repo:?}: this repository's counts are not drift against Anvil's \
             TOTAL_GATES and must not fail its PR: {:?}",
            sync.remaining_drift
        );
        assert!(
            sync.not_applicable.is_some(),
            "{repo:?}: the skip must be stated, not read as a clean page"
        );
    }
}

#[test]
fn anvil_is_recognised_case_insensitively_and_only_as_a_whole_slug() {
    // GitHub slugs are case-insensitive identities and arrive in whatever case
    // the event carried, so `Oyatie/Anvil` is Anvil. Case-insensitivity is safe
    // only because it applies to the whole slug: `ATTACKER/ANVIL` and
    // `Oyatie/Anvil-SDK` are still somebody else's. Both halves are pinned here
    // because they are one decision. See open questions — the implementer may
    // veto this in favour of an exact-case match.
    let drifting = drifting_page();

    for repo in ANVIL_SLUGS {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &drifting);

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();
        let got = std::fs::read_to_string(dir.path().join("README.md")).unwrap();

        assert_eq!(
            sync.rewritten,
            vec!["README.md".to_string()],
            "{repo} is Anvil's own repository and its counts are the sync's business"
        );
        assert!(
            got.contains(&format!("{TOTAL_GATES}-gate")),
            "{repo}: Anvil's README must be rewritten to TOTAL_GATES: {got}"
        );
        assert!(
            sync.not_applicable.is_none(),
            "{repo}: the sync applied, so it must not report otherwise: {:?}",
            sync.not_applicable
        );
    }

    for repo in ["ATTACKER/ANVIL", "Oyatie/Anvil-SDK"] {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &drifting);

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();
        let got = std::fs::read_to_string(dir.path().join("README.md")).unwrap();

        assert_eq!(
            got, drifting,
            "{repo}: ignoring case must not turn into ignoring the rest of the \
             slug; this repository is not Anvil's and must be byte-identical"
        );
        assert!(
            sync.not_applicable.is_some(),
            "{repo}: the skip must be stated, not read as a clean page"
        );
    }
}

#[test]
fn a_corpus_sync_that_did_not_apply_says_so_instead_of_passing_silently() {
    let drifting = drifting_page();

    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), &drifting);
    let anvil = sync_published_counts(ANVIL, anvil_dir.path(), TOTAL_GATES).unwrap();
    assert!(
        anvil.not_applicable.is_none(),
        "the sync did apply to Anvil, so it must not report otherwise: {:?}",
        anvil.not_applicable
    );

    for repo in WATCHED {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &drifting);

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
    // This case deliberately short-circuits on a frontmatter violation, which
    // `ensure_documentation_parity` checks after the corpus sync and before the
    // `agy` doc-parity probe. That keeps it off the probe path entirely. The
    // fence is asserted below rather than assumed: if frontmatter Rule 1 is
    // ever relaxed or reordered, this test must fail loudly instead of quietly
    // beginning to invoke a model.
    let drifting = drifting_page();
    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), &drifting);
    write(&dir.path().join("docs/doctrine.md"), &drifting);
    write(
        &dir.path().join("tenancy/policy.md"),
        "---\nstatus: active\ncanonical_authority: true\n---\n\n# Tenancy\n",
    );

    let ctx = diff_ctx("oyatie/console", &["tenancy/policy.md"]);
    let report = block_on(async {
        DocGuard::new("low".to_string())
            .ensure_documentation_parity(
                "oyatie/console",
                dir.path(),
                &ctx,
                "feat: tenancy policy",
                "",
            )
            .await
            .unwrap()
    });

    assert!(
        report.summary.contains("Frontmatter"),
        "this case is only safe while it returns through the frontmatter check, \
         which runs before the agy probe; it has stopped doing so: {}",
        report.summary
    );
    assert!(
        report.errored.is_none(),
        "the frontmatter check produced a judgement, so nothing may be Errored \
         here; an Errored report means the probe was reached: {:?}",
        report.errored
    );

    for owned in ["README.md", "docs/doctrine.md"] {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(owned)).unwrap(),
            drifting,
            "reviewing oyatie/console must not edit that repository's {owned}; \
             the edit would be committed and pushed onto the contributor's branch"
        );
    }
}

#[test]
fn the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason() {
    // Driven through the probe seam, so the summary-composing tail of
    // `ensure_documentation_parity` is reached without spawning a model.
    let drifting = drifting_page();

    // The reason the sync itself gives for the same repository. Asserting the
    // summary carries it pins that some caller *reads* `not_applicable`,
    // without dictating its wording.
    let probe_dir = tempdir().unwrap();
    write(&probe_dir.path().join("README.md"), &drifting);
    let reason = sync_published_counts("oyatie/console", probe_dir.path(), TOTAL_GATES)
        .unwrap()
        .not_applicable
        .unwrap_or_else(|| {
            panic!(
                "oyatie/console is not Anvil's repository, so the sync did not \
                 apply and must say so before any caller can repeat it"
            )
        });

    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), &drifting);
    write(&dir.path().join("docs/doctrine.md"), &drifting);

    let ctx = diff_ctx("oyatie/console", &["src/lib.rs"]);
    let sufficient = DocParityEvaluation {
        is_doc_sufficient: true,
        missing_doc_summary: None,
        doc_files_to_update: Vec::new(),
        suggested_adr_title: None,
    };
    let report = block_on(async {
        DocGuard::with_probe_override("low".to_string(), sufficient)
            .ensure_documentation_parity("oyatie/console", dir.path(), &ctx, "feat: thing", "")
            .await
            .unwrap()
    });

    assert!(
        report.is_sufficient,
        "another repository's gate counts are not Anvil's business, so a skipped \
         sync must not fail that repository's PR: {}",
        report.summary
    );
    assert!(
        report.summary.contains(&reason),
        "the gate summary must state that the corpus sync did not apply, not read \
         as a clean pass. expected it to carry {reason:?}, got: {}",
        report.summary
    );
    assert!(
        report.files_created_or_updated.is_empty(),
        "no page of another repository may be reported as touched: {:?}",
        report.files_created_or_updated
    );
    for owned in ["README.md", "docs/doctrine.md"] {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(owned)).unwrap(),
            drifting,
            "{owned} of another repository must be byte-identical"
        );
    }
}

// =========================================================================
// Issue #28 — removing the exemption removes one sentence, not one line
// =========================================================================

#[test]
fn an_exemption_marker_inside_a_table_row_leaves_the_row_and_its_neighbour_intact() {
    let (got, remaining_drift) = rewrite_anvil_readme(HISTORICAL_README_TABLE);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got}"
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

    // The whole sentence goes, not just the marker phrase. `Note: it` opens the
    // sentence and `— see the roadmap.` closes it; leaving either behind writes
    // mangled English to a published page and reports it clean.
    for orphan in [
        "Note: it",
        "such as `README.md`",
        "or `CHANGELOG.md`",
        "see the roadmap",
    ] {
        assert!(
            !got.contains(orphan),
            "{orphan:?} belongs to the exemption sentence and must go with it, \
             not survive as a fragment:\n{got}"
        );
    }
    assert_eq!(
        normalise(docguard_row),
        normalise(DOCGUARD_ROW_AFTER),
        "the row must lose exactly the exemption sentence and keep everything else"
    );
    assert!(
        docguard_row.ends_with('|'),
        "the DocGuard row must keep its closing pipe and stay a table row: {docguard_row}"
    );
    assert!(
        got.lines().any(|l| l == CEDAR_ROW),
        "the following row must not be fused into the one above it:\n{got}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn prose_following_the_exemption_sentence_on_the_same_line_survives() {
    let (got, remaining_drift) = rewrite_anvil_readme(
        "# Anvil\n\nAlpha sentence. DocGuard does **not** yet amend existing documents. Beta sentence.\nGamma line.\n",
    );

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got}"
    );
    // `DocGuard` opens the marker's sentence. A deletion that starts at the
    // marker rather than at the sentence boundary leaves it stranded.
    assert!(
        !got.contains("DocGuard"),
        "the head of the exemption sentence must go with the rest of it: {got}"
    );

    let surviving = got
        .lines()
        .find(|l| l.contains("Alpha sentence."))
        .unwrap_or_else(|| panic!("the sentence before the exemption must survive: {got}"));
    assert_eq!(
        normalise(surviving),
        vec!["Alpha sentence. Beta sentence.".to_string()],
        "removing the exemption sentence must leave its neighbours whole and on \
         one line, with nothing of the deleted sentence left behind"
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
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn the_trailing_newline_goes_only_when_the_deletion_started_at_a_line_start() {
    // Two halves of one rule, pinned together because an implementation can only
    // get them both right by looking at where `start` landed.
    //
    // The first layout — the sentence alone on its line — is the one the current
    // range computation happens to get right, and it is exactly what a rewrite
    // that narrows `end` to the sentence terminator can regress: narrow `end`
    // without keeping the newline rule and a blank line is left behind.
    let own_line = "# Anvil\n\nAlpha line.\nDocGuard does **not** yet amend existing documents.\nGamma line.\n";
    let (got, remaining_drift) = rewrite_anvil_readme(own_line);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got}"
    );
    assert_eq!(
        got.lines().count(),
        own_line.lines().count() - 1,
        "the exemption line must be removed entirely, not blanked:\n{got}"
    );
    assert_eq!(
        got.lines().filter(|l| l.trim().is_empty()).count(),
        1,
        "the only blank line is the one under the heading; no blank may be left \
         where the exemption line was:\n{got}"
    );
    assert!(
        got.contains("Alpha line.\nGamma line.\n"),
        "the neighbouring lines must be unchanged, unfused, and adjacent:\n{got}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The second layout: the marker's sentence ends the line but does not begin
    // it. The line has a surviving prefix, so its newline must stay or the
    // prefix is fused with the line below.
    let shares_a_line = "# Anvil\n\nAlpha line.\nPreamble. DocGuard does **not** yet amend existing documents.\nGamma line.\n";
    let (got, remaining_drift) = rewrite_anvil_readme(shares_a_line);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got}"
    );
    assert_eq!(
        normalise(&got),
        normalise("# Anvil\n\nAlpha line.\nPreamble.\nGamma line.\n"),
        "the surviving prefix keeps its own line; the newline is only consumed \
         when the deletion started at a line start"
    );
    assert_eq!(
        got.lines().count(),
        shares_a_line.lines().count(),
        "no line may be lost here — the exemption line had other prose on it:\n{got}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn a_multibyte_character_touching_the_deletion_boundary_is_not_split() {
    // Anvil's own corpus is not ASCII (`docs/adr/0002-…` carries Korean, and
    // the README table carries em dashes and emoji). A range computed in bytes
    // that walks one position past a sentence terminator lands mid-character
    // and panics inside `replace_range`, in the review pipeline.
    let cases: &[(&str, &str)] = &[
        // Multi-byte immediately after the sentence terminator, no space.
        (
            "Anvil does **not** yet amend existing documents.고시 detail follows.\n",
            "고시 detail follows.\n",
        ),
        (
            "Anvil does **not** yet amend existing documents.— trailing note.\n",
            "— trailing note.\n",
        ),
        (
            "Anvil does **not** yet amend existing documents.“quoted tail”.\n",
            "“quoted tail”.\n",
        ),
        // Multi-byte immediately before the deletion start.
        (
            "고시 관련. DocGuard does **not** yet amend existing documents. Beta.\n",
            "고시 관련. Beta.\n",
        ),
    ];

    for (input, expected) in cases {
        let (got, remaining_drift) = rewrite_anvil_readme(input);
        assert_eq!(
            normalise(&got),
            normalise(expected),
            "input {input:?} must lose exactly the exemption sentence"
        );
        assert_eq!(
            got.lines().count(),
            expected.lines().count(),
            "input {input:?} must keep its line structure, got {got:?}"
        );
        assert!(
            remaining_drift.is_empty(),
            "input {input:?}: {remaining_drift:?}"
        );
    }
}

#[test]
fn a_page_that_ends_at_the_exemption_sentence_is_not_overrun() {
    // No trailing newline to consume, so an `end` that walks one past the
    // sentence terminator indexes out of bounds instead of stopping. The first
    // case is the degenerate one the current code survives; the second is the
    // same file boundary with prose after the sentence, which it destroys.
    let (got, remaining_drift) =
        rewrite_anvil_readme("Anvil does **not** yet amend existing documents.");
    assert_eq!(
        got, "",
        "the page was one exemption sentence and nothing else; removing it leaves \
         nothing behind"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    let (got, remaining_drift) =
        rewrite_anvil_readme("Alpha. DocGuard does **not** yet amend existing documents. Beta.");
    assert_eq!(
        normalise(&got),
        normalise("Alpha. Beta."),
        "an unterminated last line is still a line; prose after the exemption \
         sentence survives there too"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn an_exemption_marker_at_the_start_of_the_page_takes_only_its_own_sentence() {
    let (got, remaining_drift) =
        rewrite_anvil_readme("does **not** yet amend existing documents. Beta sentence.\nGamma.\n");

    assert_eq!(
        normalise(&got),
        normalise("Beta sentence.\nGamma.\n"),
        "a marker at byte 0 has no preceding sentence to reach back to; only its \
         own sentence goes"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn every_occurrence_of_the_exemption_is_removed_not_just_the_first() {
    // `remaining_claim` fails the gate on any surviving marker, so a
    // first-occurrence-only deletion hard-fails Anvil's own gate 1 on a page
    // the sync was supposed to have fixed.
    let (got, remaining_drift) = rewrite_anvil_readme(
        "# Anvil\n\nAlpha. DocGuard does **not** yet amend existing documents. Beta.\n\
         Gamma. Anvil does **not** yet amend existing documents. Delta.\n",
    );

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "both occurrences must be removed, not only the first:\n{got}"
    );
    assert_eq!(
        normalise(&got),
        normalise("# Anvil\n\nAlpha. Beta.\nGamma. Delta.\n"),
        "each occurrence takes exactly its own sentence"
    );
    assert!(
        remaining_drift.is_empty(),
        "a surviving marker is reported as drift and fails the gate: {remaining_drift:?}"
    );
}

#[test]
fn the_plain_text_exemption_variant_is_removed_the_same_way() {
    let (got, remaining_drift) = rewrite_anvil_readme(
        "# Anvil\n\nAlpha sentence. DocGuard does not yet amend existing documents. Beta sentence.\nGamma line.\n",
    );

    assert!(
        !got.contains(PLAIN_EXEMPTION_MARKER),
        "the unbolded variant is an exemption marker too: {got}"
    );
    assert_eq!(
        normalise(&got),
        normalise("# Anvil\n\nAlpha sentence. Beta sentence.\nGamma line.\n"),
        "the unbolded variant loses exactly its own sentence"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

// =========================================================================
// Issue #29 — the public gate, not just the seam, must stop passing
// =========================================================================

#[test]
fn an_under_documented_diff_does_not_pass_through_the_public_gate() {
    // The three in-module tests drive the seam. This one drives
    // `ensure_documentation_parity` itself, so an implementation that fixes the
    // seam and leaves the entry point's hardcoded `is_sufficient: true` in
    // place cannot ship green.
    let dir = tempdir().unwrap();
    let ctx = diff_ctx(ANVIL, &["src/lib.rs"]);
    let insufficient = DocParityEvaluation {
        is_doc_sufficient: false,
        missing_doc_summary: Some("newly_public has no reference page".to_string()),
        doc_files_to_update: vec!["docs/reference/newly-public.md".to_string()],
        suggested_adr_title: None,
    };

    let report = block_on(async {
        DocGuard::with_probe_override("low".to_string(), insufficient)
            .ensure_documentation_parity(ANVIL, dir.path(), &ctx, "feat: add a public API", "")
            .await
            .unwrap()
    });

    assert!(
        !report.is_sufficient,
        "the probe judged this diff under-documented; the gate must not pass it: {}",
        report.summary
    );
    assert!(
        report.errored.is_none(),
        "a judgement was obtained and the write was possible, so this is an \
         adverse finding, not absent evidence: {:?}",
        report.errored
    );
    assert!(
        dir.path().join("docs/reference/newly-public.md").exists(),
        "the file the probe named must actually be written"
    );
    assert!(
        report
            .files_created_or_updated
            .contains(&"docs/reference/newly-public.md".to_string()),
        "the file that was written must be reported: {:?}",
        report.files_created_or_updated
    );
}
