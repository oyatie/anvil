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
//! here through `DocGuard::with_probe_override`. **No test in this file may
//! spawn a model.** `agy` is installed on developer machines and the probe is
//! invoked with `--dangerously-skip-permissions` and a 120-second budget, so
//! every case that touches `ensure_documentation_parity` is constructed through
//! the override, which makes a spawn structurally unreachable rather than
//! merely unlikely.
//!
//! ## The probe seam is part of the specification
//!
//! `DocGuard::with_probe_override` supplies the doc-parity judgement that the
//! `agy` probe would have returned. Its signature is pinned by this suite and
//! may not be changed during implementation without a fresh test review.
//!
//! The override must be consulted **inside `evaluate_doc_parity`**, at the point
//! where the probe's judgement is produced, so that an overridden run and a
//! production run traverse byte-identical code from the judgement onward. An
//! override that short-circuits earlier — returning from
//! `ensure_documentation_parity` before the corpus sync or before report
//! composition — would let every test here go green over an entry point that
//! still passes under-documented diffs in production. That is why
//! `both_probe_verdicts_carry_anvils_corpus_sync_outcome_into_the_report`,
//! `the_gate_applies_the_corpus_sync_to_anvils_own_repository` and
//! `the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason`
//! assert that work done *before* the probe (the corpus sync) and work done
//! *after* it (doc generation, summary composition) both appear in the same
//! report, on both verdicts.

use anvil::doc_guard::corpus_sync::sync_published_counts;
use anvil::doc_guard::{DocGuard, DocGuardReport, DocParityEvaluation, FrontmatterValidator};
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

/// Anvil's own repository: the only one whose published gate counts are
/// `TOTAL_GATES`.
///
/// The literal slug is deliberate. `Config::self_repo` reads `SELF_REPO` from
/// the environment (defaulting to this value), so an implementation that
/// resolves ownership through the config would make these cases depend on a
/// developer's `.env`. See open questions.
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

/// The reason a probe gives for judging a diff under-documented. Held as a
/// constant so the assertions that it reaches `DocGuardReport::summary` pin
/// pass-through rather than wording.
const MISSING_REASON: &str = "newly_public is a new public API with no reference page";

/// An Anvil page publishing a gate count that is deliberately *not*
/// `TOTAL_GATES`, so the fixture stays a drift fixture whatever `TOTAL_GATES`
/// becomes. Used only where the repository under review **is** Anvil's.
fn drifting_page() -> String {
    format!(
        "# Anvil\n\nThe fabric ships behind a {}-gate release check.\n",
        TOTAL_GATES + 1
    )
}

/// A page belonging to a repository that is **not** Anvil's.
///
/// `rewrite_page` performs three independent mutations on a page it owns: the
/// gate-count rewrite, the `sixty-gate` rewrite, and the exemption-sentence
/// deletion. A fixture whose only mutable content is a gate count would let an
/// implementation scope the count rewrite by `repo` and leave the other two
/// unscoped — which still deletes a sentence from a watched repository's README
/// and pushes it to the contributor's branch, the precise harm issue #27
/// describes. This fixture triggers all three, so `assert_eq!(got, page)` means
/// what it says: *no owned page in that repository is modified*.
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

fn sufficient() -> DocParityEvaluation {
    DocParityEvaluation {
        is_doc_sufficient: true,
        missing_doc_summary: None,
        doc_files_to_update: Vec::new(),
        suggested_adr_title: None,
    }
}

fn insufficient(reason: Option<&str>, files: &[&str]) -> DocParityEvaluation {
    DocParityEvaluation {
        is_doc_sufficient: false,
        missing_doc_summary: reason.map(|s| s.to_string()),
        doc_files_to_update: files.iter().map(|f| (*f).to_string()).collect(),
        suggested_adr_title: None,
    }
}

/// Drives the public gate with a known judgement and without a model.
fn run_gate(
    eval: DocParityEvaluation,
    repo: &str,
    repo_dir: &Path,
    changed: &[&str],
) -> DocGuardReport {
    let ctx = diff_ctx(repo, changed);
    block_on(async {
        DocGuard::with_probe_override("low".to_string(), eval)
            .ensure_documentation_parity(repo, repo_dir, &ctx, "feat: add a public API", "")
            .await
            .unwrap()
    })
}

// =========================================================================
// Issue #27 — the corpus sync is scoped to Anvil's own repository
// =========================================================================

#[test]
fn the_corpus_sync_rewrites_anvils_own_published_counts_but_not_a_watched_repositorys() {
    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), &drifting_page());

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

    let page = watched_repo_page();
    for repo in WATCHED {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in OWNED_PAGES {
            let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
            assert_eq!(
                got, page,
                "{repo}: {owned} is not Anvil's page and TOTAL_GATES says nothing \
                 about it, so the sync must leave it byte-identical — not its gate \
                 count, not its `sixty-gate` phrasing, and not its prose"
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
    let page = watched_repo_page();

    for repo in NEAR_MISSES {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in OWNED_PAGES {
            let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
            assert_eq!(
                got, page,
                "{repo:?} is not oyatie/anvil; {owned} must be byte-identical in \
                 every respect. A predicate that only separates oyatie/anvil from \
                 oyatie/console still rewrites and pushes this repository's docs"
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
    // because they are one decision, and that decision is settled: this suite
    // is the specification, so an exact-case match is out of scope for the
    // implementation.
    for repo in ANVIL_SLUGS {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &drifting_page());

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

    let page = watched_repo_page();
    for repo in ["ATTACKER/ANVIL", "Oyatie/Anvil-SDK"] {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap();

        for owned in OWNED_PAGES {
            assert_eq!(
                std::fs::read_to_string(dir.path().join(owned)).unwrap(),
                page,
                "{repo}: ignoring case must not turn into ignoring the rest of the \
                 slug; {owned} is not Anvil's and must be byte-identical"
            );
        }
        assert!(
            sync.not_applicable.is_some(),
            "{repo}: the skip must be stated, not read as a clean page"
        );
    }
}

#[test]
fn a_corpus_sync_that_did_not_apply_says_so_instead_of_passing_silently() {
    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), &drifting_page());
    let anvil = sync_published_counts(ANVIL, anvil_dir.path(), TOTAL_GATES).unwrap();
    assert!(
        anvil.not_applicable.is_none(),
        "the sync did apply to Anvil, so it must not report otherwise: {:?}",
        anvil.not_applicable
    );

    let page = watched_repo_page();
    for repo in WATCHED {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &page);

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
    // Driven through the probe seam, which replaces only the doc-parity probe:
    // the corpus-sync call this case is actually about is still the real one.
    // That makes an `agy` spawn structurally unreachable here rather than
    // merely improbable — the previous version of this case relied on the
    // frontmatter check short-circuiting first, and detected a breach of that
    // assumption only *after* the model had already run.
    let page = watched_repo_page();
    let dir = tempdir().unwrap();
    for owned in OWNED_PAGES {
        write(&dir.path().join(owned), &page);
    }
    let policy = "---\nstatus: active\ncanonical_authority: true\n---\n\n# Tenancy\n";
    write(&dir.path().join("tenancy/policy.md"), policy);

    // The fence, asserted against the validator before the guard runs rather
    // than read out of the guard's summary afterwards: this case is about the
    // frontmatter early-return path, and if the fixture stops taking that path
    // the failure must name the reason rather than the wording of an unrelated
    // validator's message.
    assert!(
        FrontmatterValidator::validate_doc_frontmatter("tenancy/policy.md", policy, dir.path())
            .is_err(),
        "this case pins the report composed on the frontmatter early-return path; \
         the fixture no longer takes it"
    );

    let report = run_gate(
        sufficient(),
        "oyatie/console",
        dir.path(),
        &["tenancy/policy.md"],
    );

    assert!(
        !report.is_sufficient,
        "the frontmatter violation is a real adverse finding: {}",
        report.summary
    );
    assert!(
        report.errored.is_none(),
        "the frontmatter check produced a judgement, so nothing may be Errored: {:?}",
        report.errored
    );

    for owned in OWNED_PAGES {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(owned)).unwrap(),
            page,
            "reviewing oyatie/console must not edit that repository's {owned}; \
             the edit would be committed and pushed onto the contributor's branch"
        );
        assert!(
            !report.files_created_or_updated.contains(&owned.to_string()),
            "no owned page of another repository may be reported as touched: {:?}",
            report.files_created_or_updated
        );
    }

    // Stated exclusion, so this is a decision rather than an omission: the
    // frontmatter early return is NOT required to carry the skipped sync's
    // reason. Issue #27's requirement is that a skipped sync must never read as
    // a silent *pass*; this path is already a stated failure with its own
    // reason, so there is nothing here for the skip to be mistaken for. The two
    // paths that can read as a pass — the sufficient and insufficient probe
    // verdicts — are pinned in
    // `the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason`.
}

#[test]
fn the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason() {
    // Driven through the probe seam, so the summary-composing tails of
    // `ensure_documentation_parity` are reached without spawning a model. Both
    // probe verdicts are exercised: an implementation that appends the reason
    // only inside the `is_doc_sufficient` branch leaves a non-Anvil repository
    // with an under-documented diff reading as though the sync had applied.
    let page = watched_repo_page();

    // The reason the sync itself gives for the same repository. Asserting the
    // summary carries it pins that some caller *reads* `not_applicable`,
    // without dictating its wording. It is derived in a different tempdir than
    // the gate runs in, which requires the reason to be path-independent.
    let probe_dir = tempdir().unwrap();
    write(&probe_dir.path().join("README.md"), &page);
    let reason = sync_published_counts("oyatie/console", probe_dir.path(), TOTAL_GATES)
        .unwrap()
        .not_applicable
        .unwrap_or_else(|| {
            panic!(
                "oyatie/console is not Anvil's repository, so the sync did not \
                 apply and must say so before any caller can repeat it"
            )
        });

    for eval in [
        sufficient(),
        insufficient(Some(MISSING_REASON), &["docs/reference/newly-public.md"]),
    ] {
        let verdict = eval.is_doc_sufficient;
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let report = run_gate(eval, "oyatie/console", dir.path(), &["src/lib.rs"]);

        assert_eq!(
            report.is_sufficient, verdict,
            "is_doc_sufficient={verdict}: another repository's gate counts are not \
             Anvil's business, so a skipped sync must neither fail nor rescue that \
             repository's PR: {}",
            report.summary
        );
        assert!(
            report.summary.contains(&reason),
            "is_doc_sufficient={verdict}: the gate summary must state that the \
             corpus sync did not apply, not read as though it had. expected it to \
             carry {reason:?}, got: {}",
            report.summary
        );
        for owned in OWNED_PAGES {
            assert_eq!(
                std::fs::read_to_string(dir.path().join(owned)).unwrap(),
                page,
                "is_doc_sufficient={verdict}: {owned} of another repository must be \
                 byte-identical"
            );
            assert!(
                !report.files_created_or_updated.contains(&owned.to_string()),
                "is_doc_sufficient={verdict}: no owned page of another repository \
                 may be reported as touched: {:?}",
                report.files_created_or_updated
            );
        }
    }
}

#[test]
fn the_gate_applies_the_corpus_sync_to_anvils_own_repository() {
    // The mirror of the scoping cases above, and the reason it exists: every
    // other #27 case calls `sync_published_counts` directly, so nothing would
    // oblige `ensure_documentation_parity` to keep calling it at all. Deleting
    // that call, or putting it behind a condition the gate never satisfies, is
    // the cheapest way to make the scoping cases green — and it would silently
    // remove Anvil's own published-count enforcement from gate 1.
    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), &drifting_page());

    let report = run_gate(sufficient(), ANVIL, dir.path(), &["src/lib.rs"]);

    let readme = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
    assert!(
        readme.contains(&format!("{TOTAL_GATES}-gate")),
        "the gate itself must apply the sync to Anvil's own README: {readme}"
    );
    assert!(
        !readme.contains(&format!("{}-gate", TOTAL_GATES + 1)),
        "the drifting claim must be gone from Anvil's own README: {readme}"
    );
    assert!(
        report
            .files_created_or_updated
            .contains(&"README.md".to_string()),
        "the page the gate rewrote must be reported: {:?}",
        report.files_created_or_updated
    );
    assert!(
        report.is_sufficient,
        "the drift was repaired and the probe judged the diff documented: {}",
        report.summary
    );
    assert!(
        report.summary.contains("README.md"),
        "the gate must state the rewrite it performed on Anvil's own page: {}",
        report.summary
    );
}

#[test]
fn both_probe_verdicts_carry_anvils_corpus_sync_outcome_into_the_report() {
    // The corpus sync runs before the probe; doc generation and summary
    // composition run after it. Requiring both to appear in the same report, on
    // both verdicts, is what stops the probe seam from short-circuiting report
    // composition: an override that returns early skips the sync, and an
    // override wired only into the sufficient branch skips the second case.
    for eval in [
        sufficient(),
        insufficient(Some(MISSING_REASON), &["docs/reference/newly-public.md"]),
    ] {
        let verdict = eval.is_doc_sufficient;
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &drifting_page());

        let report = run_gate(eval, ANVIL, dir.path(), &["src/lib.rs"]);

        let readme = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
        assert!(
            readme.contains(&format!("{TOTAL_GATES}-gate")),
            "is_doc_sufficient={verdict}: Anvil's own sync must run whatever the \
             probe went on to say: {readme}"
        );
        assert!(
            report
                .files_created_or_updated
                .contains(&"README.md".to_string()),
            "is_doc_sufficient={verdict}: the rewritten page must be reported on \
             both verdicts: {:?}",
            report.files_created_or_updated
        );
        assert_eq!(
            report.is_sufficient, verdict,
            "is_doc_sufficient={verdict}: the gate's verdict must follow the \
             probe's: {}",
            report.summary
        );
        assert!(
            report.errored.is_none(),
            "is_doc_sufficient={verdict}: a judgement was obtained and the \
             directory is writable, so nothing is absent evidence: {:?}",
            report.errored
        );
    }
}

// =========================================================================
// Issue #28 — removing the exemption removes one sentence, not one line
// =========================================================================
//
// The rule these cases collectively pin, stated once so the implementer is not
// reverse-engineering it from failures:
//
//   * A sentence is bounded by `.`, `?`, `!`, `。`, by a markdown cell
//     delimiter `|`, by a newline, or by the ends of the page.
//   * `.` is a terminator unless the next character is ASCII alphanumeric —
//     this is what keeps `README.md` and `CHANGELOG.md` from ending the
//     sentence mid-word. The exception is specific to ASCII `.`; `。` is a
//     terminator regardless of what follows it, because Korean and Japanese
//     prose does not put a space after it.
//   * A terminator belongs to the sentence it ends; `|` and `\n` are clamps and
//     stay where they are.
//   * `start` walks back from the marker to the nearest boundary, then forward
//     over spaces. `end` runs to the nearest boundary after the marker.
//   * The trailing newline is consumed only when `start` landed at a line start
//     *and* nothing survives on that line after `end` — otherwise the surviving
//     prefix or suffix would be fused with the next line.
//   * Every occurrence is removed, not only the first.

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
fn an_exemption_sentence_with_no_terminator_is_clamped_and_takes_nothing_beyond_it() {
    // The historical row happens to end its cell with `.` before the closing
    // pipe. A cell that simply runs to the pipe, and a line that simply runs to
    // the newline, carry no terminator at all — the commoner markdown shape,
    // and the one where "delete to the next `. `" deletes the rest of the file.
    let table = "| Gate | It does **not** yet amend existing documents |\n\
                 | Next | Row survives |\n";
    let (got, remaining_drift) = rewrite_anvil_readme(table);

    assert!(
        !got.contains(EXEMPTION_MARKER),
        "the exemption marker is the thing being removed: {got:?}"
    );
    let rows: Vec<&str> = got.lines().collect();
    assert_eq!(
        rows.len(),
        2,
        "neither row may be swallowed or fused: {got:?}"
    );
    assert!(
        rows[0].starts_with("| Gate |"),
        "the row's untouched first cell must survive: {:?}",
        rows[0]
    );
    assert!(
        rows[0].ends_with('|'),
        "the row must keep its closing pipe and stay a table row: {:?}",
        rows[0]
    );
    assert_eq!(
        rows[1], "| Next | Row survives |",
        "the following row must be untouched: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The same clamp in plain prose. The marker opens the line, so the emptied
    // line goes with its newline — but the line below it is not part of the
    // sentence and must survive whole.
    let (got, remaining_drift) = rewrite_anvil_readme(
        "DocGuard does **not** yet amend existing documents\nNext line here. Tail.\n",
    );
    assert_eq!(
        normalise(&got),
        normalise("Next line here. Tail.\n"),
        "an unterminated sentence ends at the newline; the following line is a \
         different sentence and survives whole: {got:?}"
    );
    assert!(
        got.lines().any(|l| l == "Next line here. Tail."),
        "the following line must stay a line of its own: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // And mid-line, where a surviving prefix keeps the line alive.
    let (got, remaining_drift) = rewrite_anvil_readme(
        "Alpha. DocGuard does **not** yet amend existing documents\nNext line here. Tail.\n",
    );
    assert_eq!(
        normalise(&got),
        normalise("Alpha.\nNext line here. Tail.\n"),
        "the prefix keeps its own line and the line below is untouched: {got:?}"
    );
    assert_eq!(
        got.lines().count(),
        2,
        "no line may be lost or fused: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");
}

#[test]
fn a_sentence_before_the_exemption_survives_whatever_terminates_it() {
    // The start boundary is a SENTENCE boundary, not a period boundary. A start
    // that scans back only for `['.', '\n']` — the shape the current code has,
    // and the one a repair that touches only `end` leaves in place — swallows
    // the contributor's preceding sentence whole and reports the page clean.
    // Anvil's own corpus carries Korean, so the ideographic full stop is not an
    // exotic case here.
    let cases: &[(&str, &str)] = &[
        (
            "Is that so? DocGuard does **not** yet amend existing documents. Beta.\n",
            "Is that so? Beta.\n",
        ),
        (
            "Wonderful! DocGuard does **not** yet amend existing documents. Beta.\n",
            "Wonderful! Beta.\n",
        ),
        // Korean and Japanese prose puts no space after `。`, so the exemption
        // sentence starts flush against the sentence before it. The marker's
        // sentence ends this line, which keeps the case about the START
        // boundary alone: there is no junction after the deletion for a
        // whitespace convention to argue about.
        (
            "고시 관련입니다。DocGuard does **not** yet amend existing documents.\nBeta.\n",
            "고시 관련입니다。\nBeta.\n",
        ),
    ];

    for (input, expected) in cases {
        let (got, remaining_drift) = rewrite_anvil_readme(input);
        assert!(
            !got.contains(EXEMPTION_MARKER),
            "input {input:?}: the marker is the thing being removed, got {got:?}"
        );
        assert_eq!(
            normalise(&got),
            normalise(expected),
            "input {input:?} must lose exactly the exemption sentence and leave the \
             sentence before it whole"
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
// Issue #29 — absent or failed evidence is never a pass
// =========================================================================
//
// Every case below drives the public entry point. The behaviours are only
// meaningful there: a helper that composes an honest report is worth nothing if
// `ensure_documentation_parity` is not obliged to call it.

#[test]
fn an_under_documented_diff_does_not_pass_through_the_public_gate() {
    let dir = tempdir().unwrap();
    let report = run_gate(
        insufficient(Some(MISSING_REASON), &["docs/reference/newly-public.md"]),
        ANVIL,
        dir.path(),
        &["src/lib.rs"],
    );

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
    assert!(
        report.summary.contains(MISSING_REASON),
        "a failing gate must state the reason it failed, not render as a list of \
         files it generated: {}",
        report.summary
    );
}

#[test]
fn an_under_documented_diff_that_named_no_files_still_fails_the_gate() {
    // The probe returned a prose finding and no file list — a common LLM output
    // shape. `is_sufficient: eval.doc_files_to_update.is_empty()` ("the gate
    // passes once the guard has nothing left to do") satisfies every other #29
    // case here and passes this diff.
    let dir = tempdir().unwrap();
    let report = run_gate(
        insufficient(Some(MISSING_REASON), &[]),
        ANVIL,
        dir.path(),
        &["src/lib.rs"],
    );

    assert!(
        !report.is_sufficient,
        "the probe judged the diff under-documented; naming no file to fix does \
         not turn that judgement into a pass: {}",
        report.summary
    );
    assert!(
        report.errored.is_none(),
        "a judgement was obtained, so this is an adverse finding and not absent \
         evidence: {:?}",
        report.errored
    );
    assert!(
        report.summary.contains(MISSING_REASON),
        "with no files to list, the summary has nothing to say unless it states \
         the probe's finding: {}",
        report.summary
    );
    assert!(
        report.files_created_or_updated.is_empty(),
        "no file was named and none was written, so none may be reported: {:?}",
        report.files_created_or_updated
    );
}

#[test]
fn an_under_documented_diff_that_stated_no_reason_still_fails_the_gate() {
    // The complement of the case above:
    // `is_sufficient: eval.missing_doc_summary.is_none()` survives that one
    // identically and dies here.
    let dir = tempdir().unwrap();
    let report = run_gate(
        insufficient(None, &["docs/reference/newly-public.md"]),
        ANVIL,
        dir.path(),
        &["src/lib.rs"],
    );

    assert!(
        !report.is_sufficient,
        "the probe judged the diff under-documented; declining to explain why \
         does not turn that judgement into a pass: {}",
        report.summary
    );
    assert!(
        report.errored.is_none(),
        "a judgement was obtained, so this is an adverse finding: {:?}",
        report.errored
    );
}

#[test]
fn a_documentation_write_that_failed_is_errored_and_never_reported_as_updated() {
    let dir = tempdir().unwrap();
    // `reference` is a regular file, so no file under `reference/` can be
    // created: every write to that path fails. The path is deliberately outside
    // the corpus sync's owned set, so the sync cannot be the source of the
    // error this case is about.
    std::fs::write(dir.path().join("reference"), "not a directory\n").unwrap();
    assert!(
        sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).is_ok(),
        "fence: the corpus sync must succeed here, so the only thing that can \
         fail is the documentation write"
    );

    let report = run_gate(
        insufficient(Some(MISSING_REASON), &["reference/newly-public.md"]),
        ANVIL,
        dir.path(),
        &["src/lib.rs"],
    );

    assert!(
        !dir.path().join("reference/newly-public.md").exists(),
        "precondition: the write cannot have succeeded"
    );
    assert!(
        report.errored.is_some(),
        "a write that failed is absent evidence and must be Errored. summary was: {}",
        report.summary
    );
    assert!(
        !report.is_sufficient,
        "a failed write must not pass the gate. summary was: {}",
        report.summary
    );
    assert!(
        report.files_created_or_updated.is_empty(),
        "nothing was written, so nothing may be reported as AutoUpdated: {:?}",
        report.files_created_or_updated
    );
}

#[test]
fn a_write_that_failed_is_not_reported_as_updated_even_when_another_one_succeeded() {
    // The partial case, decided here rather than left to the implementer: the
    // file that failed must never appear in the updated list, and the failure
    // must reach `errored`. Whether the file that *did* write may still be
    // listed is deliberately left free — either answer is honest — so nothing
    // below asserts anything about `docs/reference/written.md`.
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("reference"), "not a directory\n").unwrap();
    assert!(
        sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).is_ok(),
        "fence: the corpus sync must succeed here"
    );

    let report = run_gate(
        insufficient(
            Some(MISSING_REASON),
            &["docs/reference/written.md", "reference/unwritable.md"],
        ),
        ANVIL,
        dir.path(),
        &["src/lib.rs"],
    );

    assert!(
        !dir.path().join("reference/unwritable.md").exists(),
        "precondition: that write cannot have succeeded"
    );
    assert!(
        report.errored.is_some(),
        "one of the writes failed, and a failure that is averaged away with a \
         success is absent evidence: {}",
        report.summary
    );
    assert!(
        !report.is_sufficient,
        "a failed write must not pass the gate: {}",
        report.summary
    );
    assert!(
        !report
            .files_created_or_updated
            .contains(&"reference/unwritable.md".to_string()),
        "the file that was never written must never be reported as updated: {:?}",
        report.files_created_or_updated
    );
}

#[test]
fn naming_an_existing_file_that_is_never_amended_cannot_yield_a_pass() {
    let dir = tempdir().unwrap();
    let readme = dir.path().join("README.md");
    let before = "# Watched\n\nNothing here mentions newly_public.\n";
    std::fs::write(&readme, before).unwrap();

    let report = run_gate(
        // One file that does not exist and one that does. Creating a stub for
        // the first must not license passing on the second.
        insufficient(
            Some(MISSING_REASON),
            &["docs/reference/newly-public.md", "README.md"],
        ),
        ANVIL,
        dir.path(),
        &["src/lib.rs"],
    );

    let after = std::fs::read_to_string(&readme).unwrap();

    // Unconditional: whatever the guard decides to do about an existing file it
    // was told to update, clobbering the contributor's prose is not one of the
    // options. "Amending" by overwriting with a generated stub is the same
    // vandalism class as issues #27 and #28.
    assert!(
        after.contains("# Watched"),
        "the existing README's heading must survive: {after:?}"
    );
    assert!(
        after.contains("Nothing here mentions newly_public."),
        "the existing README's prose must survive: {after:?}"
    );

    // Unconditional too: the probe judged the diff under-documented, so neither
    // branch below can be a pass.
    assert!(
        !report.is_sufficient,
        "the probe judged the diff under-documented: {}",
        report.summary
    );

    // Two legitimate outcomes, and both have to be honest.
    if after == before {
        assert!(
            !report
                .files_created_or_updated
                .contains(&"README.md".to_string()),
            "README.md is byte-identical, so it must not be reported as updated: {:?}",
            report.files_created_or_updated
        );
    } else {
        assert!(
            report
                .files_created_or_updated
                .contains(&"README.md".to_string()),
            "README.md was amended, so it must be reported as updated: {:?}",
            report.files_created_or_updated
        );
        assert!(
            after.contains("newly_public"),
            "README.md was named because `newly_public` is undocumented; an \
             amendment that never mentions it has not closed the gap it was \
             named for, and reporting it as updated is the same false assurance: \
             {after:?}"
        );
    }
}
