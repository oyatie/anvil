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
//! `DocGuard::with_probe_override` supplies the *outcome* the `agy` probe would
//! have produced — `Ok(judgement)` or `Err(reason)`. Its signature is pinned by
//! this suite and may not be changed during implementation without a fresh test
//! review.
//!
//! The `Err` arm carries as much weight as the `Ok` arm. "Absent or failed
//! evidence is never a pass" is a statement about a probe that produced *no*
//! judgement — spawn failure, non-zero exit, timeout, unparseable JSON,
//! watchdog supervision failure — and that is the arm whose historical collapse
//! into `is_doc_sufficient: true` made gate 1 unfailable (the comment recording
//! it still sits in `evaluate_doc_parity`). A seam that could only express a
//! successful judgement would leave the arm reachable only from production, so
//! `Err(reason)` must be delivered as an `Err` out of `evaluate_doc_parity` —
//! the same path a real probe failure takes.
//!
//! The override must be consulted **inside `evaluate_doc_parity`**, at the point
//! where the probe's outcome is produced, so that an overridden run and a
//! production run traverse byte-identical code from that outcome onward. An
//! override that short-circuits earlier — returning from
//! `ensure_documentation_parity` before the corpus sync or before report
//! composition — would let every test here go green over an entry point that
//! still passes under-documented diffs in production. That is why
//! `both_probe_verdicts_carry_anvils_corpus_sync_outcome_into_the_report`,
//! `the_gate_applies_the_corpus_sync_to_anvils_own_repository` and
//! `the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason`
//! assert that work done *before* the probe (the corpus sync) and work done
//! *after* it (doc generation, summary composition) both appear in the same
//! report, on both verdicts — and why
//! `a_failed_probe_is_not_rescued_by_a_corpus_sync_that_did_have_work_to_do`
//! asserts the sync's disk effect on the `Err` arm, which is the only arm where
//! that fence was previously missing.
//!
//! ## The override is a stored value, not a slot that empties
//!
//! `with_probe_override` stores an outcome that every call to
//! `ensure_documentation_parity` on that guard observes. It is **not** a
//! one-shot slot: an implementation that `take()`s it, or that otherwise stops
//! answering after the first call, must never fall through to the real `agy`
//! spawn — there is no "override exhausted" state in which spawning becomes
//! legal. Falling through would make `agy --dangerously-skip-permissions`
//! reachable from `cargo test` the moment anything calls the gate twice on one
//! guard (a retry loop, a shared guard, a second gate invocation), which is the
//! outcome the paragraph above claims has been made structurally impossible.
//!
//! That is settled in two places, neither of which is this binary.
//! `DocGuard`'s stored outcome is declared as `doc_guard::Probe` — an enum with
//! no empty arm and no drainable arm — so "the override has been used up" is a
//! state the implementation cannot spell; and
//! `tests/docguard_oracle_repair_probe_seam_test.rs` pins the behaviour by
//! running the gate twice on one guard and requiring the two reports to agree.
//! It lives in its own binary because it neutralises `PATH` first, so that a
//! fall-through spawn fails in microseconds instead of invoking a model — the
//! detection mechanism must not be the act it forbids.
//!
//! ## Four cases in this file are GREEN at review time, deliberately
//!
//! The last section pins the `DocGuardReport` -> `GateStatus` mapping, which is
//! where issue #29's requirement is actually decided. To drive it, the
//! scaffolding EXTRACTED the evaluator's existing inline mapping into
//! `pre_merge_guard::evaluator::doc_parity_status` — verbatim, defect and all —
//! rather than replacing it with a `todo!()`. The consequence is that four of
//! the five cases there pass today:
//!
//! * `a_diff_..._does_not_certify_when_no_file_was_written`
//! * `a_probe_that_produced_no_judgement_does_not_certify`
//! * `an_errored_gate_does_not_certify_even_when_a_page_was_rewritten`
//! * `a_sufficient_diff_certifies_and_a_rewritten_owned_page_does_not_block_it`
//!
//! They are regression fences on arms of the mapping that are already correct
//! and that the repair for the fifth case must not break — the last one is the
//! counterweight that stops "never accept a non-empty file list" from being the
//! cheapest repair. Their falsifiability was checked by mutation rather than
//! assumed: dropping the `Errored` arm fails two of them, turning the
//! `AutoUpdated` arm into `Failed` fails the fourth, and blocking with an empty
//! reason fails the fifth (red) case's pass-through assertion.
//!
//! Had the seam been given a `todo!()` body instead, all five would be red — on
//! a panic, proving only that a function is unimplemented, and hiding the one
//! thing worth showing: that the mapping certifies an under-documented diff
//! **today**, which is what
//! `a_diff_..._does_not_certify_because_a_stub_was_written` reports when it
//! fails with `status: AutoUpdated`.
//!
//! ## Ownership is a compile-time constant
//!
//! Which repository is Anvil's own is settled by this suite as a property of the
//! build, not of the process environment. The companion binary
//! `tests/docguard_oracle_repair_self_repo_test.rs` pins it: with
//! `SELF_REPO=oyatie/console` set, `oyatie/console` is still skipped and
//! `oyatie/anvil` is still synced. It lives in its own test binary because it
//! mutates the environment, which is a data race in a parallel one.

use anvil::doc_guard::corpus_sync::sync_published_counts;
use anvil::doc_guard::{DocGuard, DocGuardReport, DocParityEvaluation, FrontmatterValidator};
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::evaluator::doc_parity_status;
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use std::path::Path;
use tempfile::tempdir;

/// Anvil's own repository: the only one whose published gate counts are
/// `TOTAL_GATES`.
///
/// The literal slug is deliberate, and it is a decision rather than a deferral.
/// `Config::self_repo` reads `SELF_REPO` from the environment, and
/// `Config::from_env` calls `dotenvy::dotenv()`, which mutates the process
/// environment. Resolving ownership that way would make these cases depend on
/// whatever `.env` sits in the developer's working directory and would make a
/// mis-set `SELF_REPO` rewrite and push a watched repository's docs — issue #27
/// verbatim. See `tests/docguard_oracle_repair_self_repo_test.rs`.
///
/// STATED CONSEQUENCE of that decision, recorded so it reads as a known cost
/// rather than an oversight: ownership is a compile-time constant in the
/// implementation too, so renaming or moving this repository switches gate 1's
/// corpus enforcement off — silently, with this whole suite still green, because
/// every case here supplies the slug it expects. The failure direction is safe
/// (Anvil stops repairing its own pages; no watched repository is ever
/// corrupted), and a test cannot distinguish "renamed" from "correctly scoped"
/// without inventing a second source of truth for the name. A rename must
/// therefore update the constant in the same commit.
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

/// The five ways `evaluate_doc_parity` can come back with no judgement at all.
/// The strings are the shapes the real call site produces (spawn failure,
/// non-zero exit, timeout, unparseable output, watchdog supervision failure);
/// their wording is the test's, and it is only ever asserted as pass-through.
const PROBE_FAILURES: &[&str] = &[
    "failed to run doc parity probe: No such file or directory (os error 2)",
    "doc parity probe exited with status exit status: 1: permission check failed for command",
    "doc parity probe timed out after 120s",
    "doc parity probe returned no parseable evaluation (stdout 4096 bytes)",
    "doc parity probe supervision failed: watchdog channel closed",
];

/// An Anvil page publishing claims that are deliberately *not* `TOTAL_GATES`,
/// so the fixture stays a drift fixture whatever `TOTAL_GATES` becomes. Used
/// only where the repository under review **is** Anvil's.
///
/// Both of `rewrite_page`'s count mutations are represented: the numeric
/// gate-count rewrite and the `sixty-gate` rewrite. The second has no other
/// positive coverage anywhere — the in-module unit tests only use the digit
/// form, and the live corpus contains no `sixty-gate` occurrence for
/// `published_*_matches_live_corpus` to catch — so without it an implementer
/// scoping `rewrite_page` could delete the `sixty_regex` replacement outright
/// while keeping `remaining_claim`'s `sixty-gate` check, leaving Anvil's own
/// gate 1 to hard-fail the moment any owned page reintroduces the word.
fn drifting_page() -> String {
    format!(
        "# Anvil\n\
         \n\
         The fabric ships behind a {}-gate release check.\n\
         It replaced the sixty-gate pilot programme.\n",
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

/// A page with nothing at all for the sync to do: its published count already
/// is `TOTAL_GATES`, it carries no `sixty-gate` and no exemption marker.
///
/// An applied sync and a skipped sync therefore have exactly the same work to
/// report on it — none — which is what makes the two summaries comparable in
/// `an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply`.
fn already_honest_page() -> String {
    format!("# Page\n\nShips behind a {TOTAL_GATES}-gate release check.\n")
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

/// The reason the sync gives for declining to apply to a repository that is not
/// Anvil's, read back from the sync itself so every assertion about it pins
/// pass-through rather than wording.
///
/// STATED REQUIREMENT: the reason is a property of *which repository is under
/// review*, not of what happens to be on disk. It is derived here in a tempdir
/// other than the one the gate under test runs in, so a reason that enumerated
/// the pages it declined to touch, or that named their paths, would not satisfy
/// this suite. That is a requirement, not an artefact of how the fixture was
/// built.
fn skipped_sync_reason(repo: &str) -> String {
    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), &watched_repo_page());
    let reason = sync_published_counts(repo, dir.path(), TOTAL_GATES)
        .unwrap()
        .not_applicable
        .unwrap_or_else(|| {
            panic!(
                "{repo:?} is not Anvil's repository, so the sync did not apply and \
                 must say so before any caller can repeat it"
            )
        });
    assert!(
        !reason.trim().is_empty(),
        "{repo:?}: the stated reason must actually say something"
    );
    reason
}

fn diff_ctx(repo: &str, repo_dir: &Path, changed: &[&str]) -> PrDiffContext {
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
        // The checkout the diff came from is the checkout under review, which is
        // this test's tempdir. Pointing it at `"."` would point it at the
        // developer's own Anvil checkout: an implementation that decided to sync
        // `diff_ctx.repo_working_dir` would then rewrite gate counts in, and
        // delete exemption sentences from, the live README.md, docs/doctrine.md,
        // openapi/openapi.yaml and docs/adr/*.md of the repository under test —
        // from several test threads at once — before any assertion caught it.
        repo_working_dir: repo_dir.to_path_buf(),
    }
}

fn block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

fn sufficient() -> Result<DocParityEvaluation, String> {
    Ok(DocParityEvaluation {
        is_doc_sufficient: true,
        missing_doc_summary: None,
        doc_files_to_update: Vec::new(),
        suggested_adr_title: None,
    })
}

fn insufficient(reason: Option<&str>, files: &[&str]) -> Result<DocParityEvaluation, String> {
    Ok(DocParityEvaluation {
        is_doc_sufficient: false,
        missing_doc_summary: reason.map(|s| s.to_string()),
        doc_files_to_update: files.iter().map(|f| (*f).to_string()).collect(),
        suggested_adr_title: None,
    })
}

/// The probe came back with no judgement at all.
fn probe_failed(reason: &str) -> Result<DocParityEvaluation, String> {
    Err(reason.to_string())
}

/// `true` when the outcome is a judgement of sufficiency. Only meaningful for
/// the `Ok` arm; used to label the two-verdict loops.
fn verdict_of(outcome: &Result<DocParityEvaluation, String>) -> bool {
    outcome
        .as_ref()
        .map(|e| e.is_doc_sufficient)
        .unwrap_or(false)
}

/// Drives the public gate with a known probe outcome and without a model.
fn run_gate(
    outcome: Result<DocParityEvaluation, String>,
    repo: &str,
    repo_dir: &Path,
    changed: &[&str],
) -> DocGuardReport {
    let ctx = diff_ctx(repo, repo_dir, changed);
    block_on(async {
        DocGuard::with_probe_override("low".to_string(), outcome)
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
    // The page published two gate-count claims — `TOTAL_GATES + 1` in digits and
    // `sixty-gate` spelled out — and both are claims about the same number. Both
    // must still be *there* afterwards, and both must now read `TOTAL_GATES`.
    //
    // Counted rather than matched literally, and case-insensitively, because the
    // behaviour is "every published claim now states TOTAL_GATES", not "the
    // rewriter capitalises the way today's `sixty_regex` replacement happens to".
    // A rewriter that normalises `sixty-gate` to `72-gate` instead of `72-Gate`
    // is different but correct and must pass; a rewriter that *deletes* the
    // spelled-out claim rather than repairing it is not, and the count is what
    // separates them — `!contains("sixty-gate")` alone accepts deletion, and
    // `contains("{TOTAL_GATES}-gate")` alone is already satisfied by the digit
    // claim's repair.
    let lowered = anvil_readme.to_lowercase();
    let repaired = format!("{TOTAL_GATES}-gate");
    assert_eq!(
        lowered.matches(&repaired).count(),
        2,
        "both published gate-count claims must survive the rewrite and both must \
         now read TOTAL_GATES={TOTAL_GATES}; deleting a claim is not repairing \
         it: {anvil_readme}"
    );
    assert!(
        !lowered.contains(&format!("{}-gate", TOTAL_GATES + 1)),
        "the drifting digit claim must be gone: {anvil_readme}"
    );
    // `remaining_claim` fails the gate on a surviving `sixty-gate`, so a rewriter
    // that stops repairing it turns the word into an unfixable hard failure on
    // Anvil's own PRs.
    assert!(
        !lowered.contains("sixty-gate"),
        "no page of Anvil's may go on publishing `sixty-gate`: {anvil_readme}"
    );
    assert!(
        anvil.remaining_drift.is_empty(),
        "every claim on this page is one the rewriter knows how to repair: {:?}",
        anvil.remaining_drift
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
        let reason = sync.not_applicable.as_deref().unwrap_or_else(|| {
            panic!("{repo:?}: the skip must be stated, not read as a clean page")
        });
        assert!(
            !reason.trim().is_empty(),
            "{repo:?}: the stated reason must actually say something — \
             `Some(String::new())` states nothing, and the empty slug is exactly \
             where a reason built out of the slug degenerates to it"
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
        let reason = sync
            .not_applicable
            .as_deref()
            .unwrap_or_else(|| panic!("{repo}: the skip must be stated, not read as a clean page"));
        assert!(
            !reason.trim().is_empty(),
            "{repo}: the stated reason must actually say something"
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

    // The fail-closed mirror, and the one every fixture above misses: each of
    // them hands the sync a clean, fully readable tempdir, so "the sync did not
    // apply" and "the sync applied and found nothing to do" are still
    // distinguishable only by `not_applicable`. A corpus the sync CANNOT READ in
    // a repository that is not Anvil's separates them properly.
    //
    // The wrong implementation this catches is the natural edit, not a contrived
    // one: apply the ownership predicate per page rather than as an early
    // return, because that is where you already are when you reach for it —
    //
    //     for rel in pages {
    //         let original = std::fs::read_to_string(&path)?;   // runs for EVERY repo
    //         let updated = rewrite_page(&original, total_gates);
    //         if is_anvil(repo) && updated != original { write; rewritten.push(rel) }
    //         if is_anvil(repo) && let Some(why) = remaining_claim(..) { drift.push(..) }
    //     }
    //
    // Every case above still passes: the foreign pages stay byte-identical,
    // `rewritten` and `remaining_drift` are empty, `not_applicable` is `Some`.
    // But a watched repository whose `README.md` is a directory, or whose
    // `docs/adr` is unreadable, now returns `Err` from a sync that does not even
    // apply to it. `ensure_documentation_parity` maps that to `errored`, gate 1
    // goes Errored, and every pull request on that repository is blocked by
    // Anvil's private gate count — issue #27's harm arrived at from the
    // fail-closed side.
    //
    // The fixture is the one from
    // `a_corpus_sync_that_could_not_run_at_all_is_errored_at_the_gate`, where the
    // same unreadable page is (correctly) `Err` for Anvil, so the pair pins the
    // whole decision: whose corpus it is settles whether it is read at all.
    for repo in WATCHED {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("README.md")).unwrap();
        assert!(
            sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).is_err(),
            "fence: this same fixture must remain unreadable for Anvil, or the case \
             below is not about a corpus the sync could not read"
        );

        let sync = sync_published_counts(repo, dir.path(), TOTAL_GATES).unwrap_or_else(|e| {
            panic!(
                "{repo}: an unreadable page in somebody else's repository is not \
                 Anvil's business either. Reading it at all is the defect; failing \
                 the sync on it blocks every pull request on {repo} at gate 1 with \
                 Anvil's private gate count. got: {e}"
            )
        });

        assert!(
            sync.rewritten.is_empty(),
            "{repo}: nothing may be reported as rewritten: {:?}",
            sync.rewritten
        );
        assert!(
            sync.remaining_drift.is_empty(),
            "{repo}: a corpus the sync does not own is not drift against Anvil's \
             TOTAL_GATES, readable or not: {:?}",
            sync.remaining_drift
        );
        let reason = sync.not_applicable.as_deref().unwrap_or_else(|| {
            panic!("{repo}: the skip must be stated here too, not read as a clean page")
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
    let reason = skipped_sync_reason("oyatie/console");

    for outcome in [
        sufficient(),
        insufficient(Some(MISSING_REASON), &["docs/reference/newly-public.md"]),
    ] {
        let verdict = verdict_of(&outcome);
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let report = run_gate(outcome, "oyatie/console", dir.path(), &["src/lib.rs"]);

        assert_eq!(
            report.is_sufficient, verdict,
            "is_doc_sufficient={verdict}: another repository's gate counts are not \
             Anvil's business, so a skipped sync must neither fail nor rescue that \
             repository's PR: {}",
            report.summary
        );
        // A sync that does not apply is not a gate that could not judge. Every
        // pull request on every watched repository takes this path, so an
        // implementation that reports the skip as absent evidence blocks all of
        // them at gate 1 forever — the fail-closed mirror image of #27.
        assert!(
            report.errored.is_none(),
            "is_doc_sufficient={verdict}: the probe produced a judgement and the \
             skipped sync is a stated fact, not missing evidence; nothing here may \
             be Errored: {:?}",
            report.errored
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

    // The same fail-closed mirror, pinned at the GATE rather than at the sync,
    // because that is where the harm lands: `ensure_documentation_parity` maps
    // an `Err` from the sync onto `errored`, gate 1 goes Errored, and
    // `GateStatus::Errored` is not acceptable — so every pull request on this
    // repository is blocked by a corpus that is not Anvil's, was never Anvil's
    // business, and that Anvil had no reason to open. The comment above says an
    // implementation reporting the skip as absent evidence "blocks all of them at
    // gate 1 forever"; this is the fixture that makes that a test rather than a
    // remark. See the matching sync-level case in
    // `a_corpus_sync_that_did_not_apply_says_so_instead_of_passing_silently`.
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("README.md")).unwrap();
    assert!(
        sync_published_counts(ANVIL, dir.path(), TOTAL_GATES).is_err(),
        "fence: this fixture must remain unreadable for Anvil, or the case below is \
         not about a corpus the sync could not read"
    );

    let report = run_gate(sufficient(), "oyatie/console", dir.path(), &["src/lib.rs"]);

    assert!(
        report.errored.is_none(),
        "oyatie/console: the sync did not apply to this repository, so a page it \
         never had cause to read is not absent evidence about this pull request. \
         Erroring here blocks every PR on every watched repository at gate 1: {:?}",
        report.errored
    );
    assert!(
        report.is_sufficient,
        "oyatie/console: the probe judged the diff documented and the skipped sync \
         has no finding of its own to add: {}",
        report.summary
    );
    assert!(
        report.summary.contains(&reason),
        "oyatie/console: the skip must still be stated on this path — an unreadable \
         page does not turn a skipped sync into an applied one: {}",
        report.summary
    );
    assert!(
        report.files_created_or_updated.is_empty(),
        "oyatie/console: nothing was rewritten, so nothing may be reported as \
         touched: {:?}",
        report.files_created_or_updated
    );
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
        !readme.contains("sixty-gate"),
        "the spelled-out claim must be gone too: {readme}"
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
    assert!(
        !report
            .summary
            .contains(&skipped_sync_reason("oyatie/console")),
        "the sync demonstrably applied here, so the summary must not carry a \
         skipped sync's reason: {}",
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
    let skip_reason = skipped_sync_reason("oyatie/console");

    for outcome in [
        sufficient(),
        insufficient(Some(MISSING_REASON), &["docs/reference/newly-public.md"]),
    ] {
        let verdict = verdict_of(&outcome);
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &drifting_page());

        let report = run_gate(outcome, ANVIL, dir.path(), &["src/lib.rs"]);

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
        assert!(
            !report.summary.contains(&skip_reason),
            "is_doc_sufficient={verdict}: the sync applied to Anvil's own page, so \
             the summary must not carry a skipped sync's reason: {}",
            report.summary
        );
    }
}

#[test]
fn an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply() {
    // `!summary.contains(&reason)` alone does not close this. An implementation
    // that appends the skip statement unconditionally —
    // `let skip = sync.not_applicable.unwrap_or_default();` followed by an
    // unconditional `format!("{base} (corpus sync did not apply: {skip})")` —
    // satisfies it, because the reason it interpolated for Anvil is the empty
    // string. Every Anvil PR is then told the sync did not apply while it
    // demonstrably did.
    //
    // STATED REQUIREMENT, so this is a decision and not an artefact: the summary
    // of a *skipped* sync is the summary of an *applied* sync that had nothing
    // to rewrite, plus a statement of the skip. Pinning that relation, rather
    // than the wording of either, is what makes the skip statement's presence
    // observable without this suite inventing the words it is made of.
    let page = already_honest_page();
    let reason = skipped_sync_reason("oyatie/console");

    let anvil_dir = tempdir().unwrap();
    write(&anvil_dir.path().join("README.md"), &page);
    let anvil = run_gate(sufficient(), ANVIL, anvil_dir.path(), &["src/lib.rs"]);

    let console_dir = tempdir().unwrap();
    write(&console_dir.path().join("README.md"), &page);
    let console = run_gate(
        sufficient(),
        "oyatie/console",
        console_dir.path(),
        &["src/lib.rs"],
    );

    // The fixture is honest already, so neither run has anything to rewrite and
    // neither has anything adverse to report.
    for (label, report) in [("oyatie/anvil", &anvil), ("oyatie/console", &console)] {
        assert!(
            report.is_sufficient,
            "{label}: the page publishes TOTAL_GATES and the probe judged the diff \
             documented: {}",
            report.summary
        );
        assert!(
            report.errored.is_none(),
            "{label}: a judgement was obtained and nothing failed: {:?}",
            report.errored
        );
        assert!(
            report.files_created_or_updated.is_empty(),
            "{label}: nothing needed rewriting, so nothing may be reported as \
             touched: {:?}",
            report.files_created_or_updated
        );
        assert!(
            !report.summary.trim().is_empty(),
            "{label}: a gate that says nothing cannot be read"
        );
    }

    assert_eq!(
        std::fs::read_to_string(anvil_dir.path().join("README.md")).unwrap(),
        page,
        "the page already publishes TOTAL_GATES; an applied sync has nothing to do \
         to it"
    );
    assert!(
        console.summary.contains(&reason),
        "oyatie/console: the skipped sync must be stated: {}",
        console.summary
    );
    assert!(
        !anvil.summary.contains(&reason),
        "oyatie/anvil: the sync applied, so the summary must not carry a skipped \
         sync's reason: {}",
        anvil.summary
    );
    assert!(
        console.summary.contains(&anvil.summary),
        "the skipped summary must be the applied summary plus a statement of the \
         skip — nothing about the skip may appear in the applied one.\n\
         applied: {}\nskipped: {}",
        anvil.summary,
        console.summary
    );
}

#[test]
fn a_corpus_sync_that_could_not_run_at_all_is_errored_at_the_gate() {
    // The sibling of the drift arm, and the one the gate can actually be driven
    // into: the sync's adverse outcomes must survive the restructuring this
    // branch performs on that match. A refactor to
    // `let sync = sync_published_counts(..)?;` drops this arm, and Anvil's own
    // gate 1 then reports a corpus it could not even read as sufficient.
    //
    // `README.md` is a directory, so reading it fails with something that is not
    // NotFound and the sync returns `Err`.
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("README.md")).unwrap();
    let sync_error = match sync_published_counts(ANVIL, dir.path(), TOTAL_GATES) {
        Err(e) => e.to_string(),
        Ok(_) => panic!("fence: an owned page that cannot be read must fail the sync"),
    };

    let report = run_gate(sufficient(), ANVIL, dir.path(), &["src/lib.rs"]);

    assert!(
        report.errored.is_some(),
        "a corpus the gate could not read is absent evidence: {}",
        report.summary
    );
    assert!(
        !report.is_sufficient,
        "the probe said the diff was documented, but the published corpus was \
         never checked; that is not a pass: {}",
        report.summary
    );
    assert!(
        report.files_created_or_updated.is_empty(),
        "nothing was rewritten, so nothing may be reported as touched: {:?}",
        report.files_created_or_updated
    );
    assert!(
        report.summary.contains(&sync_error),
        "the gate must state what it could not do; expected the summary to carry \
         {sync_error:?}, got: {}",
        report.summary
    );
}

// STATED EXCLUSION — the corpus sync's `remaining_drift` arm at the gate.
//
// `ensure_documentation_parity` matches three ways on the sync: `Err` (absent
// evidence, pinned by the case above), non-empty `remaining_drift` (hard fail,
// not AutoUpdated), and the ordinary success. This branch restructures that
// match to thread `not_applicable` through it, and the drift arm is the one no
// case here drives. That is a decision, recorded so an implementer restructuring
// the match knows the arm is load-bearing rather than dead — not an oversight.
//
// It is not driven because it is not reachable through a *correct* rewriter, and
// the reason is structural rather than a shortage of fixtures:
//
//   * `rewrite_page` and `remaining_claim` share the same two regexes and the
//     same `EXEMPTION_MARKERS` list, so anything the checker can see is
//     something the rewriter has already normalised. The count rewrite emits
//     `TOTAL_GATES` followed by the captured suffix verbatim, so the repaired
//     claim always re-parses; the checker's "unparseable gate-count claim"
//     branch needs digits the rewriter's identical `\d+` did not match, and
//     there are none.
//   * The deletion cannot *manufacture* a claim at its junction. `start` is
//     always immediately preceded by a sentence terminator, a `|`, a newline, or
//     the start of the page — never a digit and never a partial marker — so no
//     `\d+ gates`, no `sixty-gate` and no marker can be formed by splicing the
//     surviving prefix onto the surviving suffix.
//
// So drift after a rewrite means the rewriter failed, which is a self-check on
// code this branch is replacing rather than a property of page content. That
// makes the arm a fail-closed net for layouts this suite does not carry: if the
// new sentence deletion misses an occurrence, `remaining_claim` is what turns
// that into a blocked PR instead of a page published mangled and reported clean.
// Every issue-#28 case below asserts `remaining_drift.is_empty()`, which fences
// the argument from the other side — if the new rewriter ever does leave drift
// on a pinned layout, those assertions say so.
//
// REQUIREMENT FOR THE IMPLEMENTER, since no test can enforce it: the drift arm
// must survive the restructuring. `let sync = corpus_sync::sync_published_counts(
// repo, repo_dir, TOTAL_GATES)?;` deletes it silently and is caught by the case
// above; a restructure that keeps the `Err` arm and drops only the drift guard
// is caught by nothing here. If the implementer's rewriter *can* leave a marker
// or a stale count behind in some layout, the arm becomes reachable and must be
// pinned at the gate in the same commit.

// =========================================================================
// Issue #28 — removing the exemption removes one sentence, not one line
// =========================================================================
//
// The rule these cases collectively pin, stated once so the implementer is not
// reverse-engineering it from failures:
//
//   * A sentence is bounded by `.`, `?`, `!`, `。`, by a markdown cell
//     delimiter `|`, by a newline, or by the ends of the page. The set is the
//     same walking backwards from the marker and walking forwards from it;
//     `a_sentence_before_the_exemption_survives_whatever_terminates_it` and
//     `the_exemption_sentence_ends_at_whatever_terminates_it` pin each
//     terminator on both sides, because two scan helpers with different
//     terminator sets is the shape that passes half a suite.
//   * `.` is a terminator unless the next character is ASCII alphanumeric —
//     this is what keeps `README.md`, `CHANGELOG.md` and `v1.2` from ending the
//     sentence mid-word. The exception is specific to ASCII `.`; `。` is a
//     terminator regardless of what follows it, because Korean and Japanese
//     prose does not put a space after it. `is_ascii_alphanumeric`, not
//     `is_alphanumeric`: `.고시` ends a sentence and `.md` does not.
//   * A terminator belongs to the sentence it ends; `|` and `\n` are clamps and
//     stay where they are.
//   * `start` walks back from the marker to the nearest boundary, then forward
//     over spaces. `end` runs to the nearest boundary after the marker.
//   * Ranges are byte ranges over UTF-8: a terminator that belongs to the
//     sentence is consumed by its own encoded length, never by one byte.
//   * The trailing newline is consumed only when `start` landed at a line start
//     *and* nothing survives on that line after `end` — otherwise the surviving
//     prefix or suffix would be fused with the next line.
//   * Every occurrence is removed, not only the first — including two on one
//     line, and including one of each marker variant on the same line.

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
        // The ASCII-alphanumeric exception belongs to `.` ALONE, on the START
        // side too. Both fixtures above put a space after their terminator, so a
        // backward scan that carries the `.`-only exception onto `?` and `!`
        // passes them — `。` is the only one of the three pinned flush against
        // ASCII here (`고시 관련입니다。DocGuard …`, below), and it is pinned on
        // the END side by three fixtures in
        // `the_exemption_sentence_ends_at_whatever_terminates_it`. With that
        // mutation the scan walks back past `?` to byte 0 and deletes the
        // contributor's question along with the exemption: mangled prose,
        // reported clean, which is issue #28's harm reached from the start side.
        (
            "Is that so?DocGuard does **not** yet amend existing documents. Beta.\n",
            "Is that so? Beta.\n",
        ),
        (
            "Wonderful!DocGuard does **not** yet amend existing documents. Beta.\n",
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
        // The `.`-is-not-always-a-terminator rule, on the START side. The
        // orphan assertions in the table case pin it on the END side only, so a
        // repair that touches only `end` — the cheapest possible fix for #28 —
        // leaves the naive `rfind(['.', '\n'])` start in place and passes
        // everything else. Here the LAST `.` before the marker is the one
        // inside `v1.2`, so that start lands between `1` and `2` and publishes
        // "Alpha. See CHANGELOG.md and v1 Beta." — mangled prose, reported
        // clean, which is issue #28's harm exactly.
        (
            "Alpha. See CHANGELOG.md and v1.2 for details, but DocGuard does **not** yet amend existing documents. Beta.\n",
            "Alpha. Beta.\n",
        ),
        // The same rule where the sentence terminator does follow the intra-word
        // dots, so a correct scan must walk past `CHANGELOG.md` and `v1.2` and
        // still stop at `notes.`.
        (
            "See CHANGELOG.md for v1.2 notes. DocGuard does **not** yet amend existing documents. Beta.\n",
            "See CHANGELOG.md for v1.2 notes. Beta.\n",
        ),
        // `is_ascii_alphanumeric`, not `is_alphanumeric`, on the START side.
        // The rule at the head of this section says `.고시` ends a sentence and
        // `.md` does not; until now that distinction was pinned on the END side
        // only (`a_multibyte_character_touching_the_deletion_boundary_is_not_split`
        // carries every `.`+non-ASCII fixture, and all of them sit *after* the
        // marker). A backward scan written with the obvious `is_alphanumeric` —
        // the method name the rule's own warning is about — refuses to stop at
        // the `.` after `Alpha`, walks back to byte 0, and deletes
        // `Alpha.고시 …` wholesale: a mangled page, reported clean. Anvil's own
        // corpus carries Korean (`docs/adr/0002-…`), so `.` flush against
        // Hangul is a real layout here.
        //
        // `고시` opens the marker's sentence — the terminator before it is the
        // `.` after `Alpha` — so it goes with the sentence, exactly as
        // `고시 detail follows.` survives on the END side for the mirror reason.
        (
            "Alpha.고시 DocGuard does **not** yet amend existing documents. Beta.\n",
            "Alpha. Beta.\n",
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
fn the_exemption_sentence_ends_at_whatever_terminates_it() {
    // The mirror of the case above, and the reason it exists: every other
    // END-boundary assertion in this suite terminates on `.`, `|`, `\n` or the
    // end of the page. An implementation with two scan helpers — a start scan
    // over ['.', '?', '!', '。', '|', '\n'] and an end scan over ['.', '|',
    // '\n'] — satisfies the whole of issue #28's coverage without ever handling
    // `?`, `!` or `。` at the end of the deleted sentence, and swallows the
    // following sentence when the exemption ends in one.
    let cases: &[(&str, &str)] = &[
        (
            "Alpha. It does **not** yet amend existing documents! Beta. Gamma.\n",
            "Alpha. Beta. Gamma.\n",
        ),
        (
            "Alpha. It does **not** yet amend existing documents? Beta. Gamma.\n",
            "Alpha. Beta. Gamma.\n",
        ),
        // Doubly load-bearing: `。` is three bytes, so an end scan that consumes
        // its terminator with `end += 1` instead of `end += ch.len_utf8()`
        // lands mid-character and panics inside `replace_range` — in the review
        // pipeline, on Anvil's own corpus, which carries Korean. The byte-index
        // defect is pinned today on the start side only.
        (
            "Anvil does **not** yet amend existing documents。고시 관련. Beta.\n",
            "고시 관련. Beta.\n",
        ),
        // The ASCII-alphanumeric exception belongs to `.` ALONE. `。`, `?` and
        // `!` terminate regardless of what follows them, and until now that was
        // pinned on the START side only (`고시 관련입니다。DocGuard …`, where `。`
        // is followed by ASCII `D`); every END-side fixture put a space or a
        // non-ASCII character after the terminator, so an end scan carrying the
        // `.`-only exception onto the other three passed the whole suite.
        //
        // With that mutation, `。` stops terminating here, `end` runs on to the
        // `.` after `detail`, and the contributor's next sentence is deleted —
        // issue #28's harm, reached from the terminator the rule says is
        // unconditional.
        (
            "Anvil does **not** yet amend existing documents。Beta detail. Gamma.\n",
            "Beta detail. Gamma.\n",
        ),
        (
            "Anvil does **not** yet amend existing documents!Beta detail. Gamma.\n",
            "Beta detail. Gamma.\n",
        ),
        (
            "Anvil does **not** yet amend existing documents?Beta detail. Gamma.\n",
            "Beta detail. Gamma.\n",
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
            "input {input:?}: the exemption sentence ends at its own terminator, and \
             the sentence after it survives whole"
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

    // The shape neither case above actually exercises, and the one the required
    // rewrite makes reachable: the marker's own sentence runs to end-of-page with
    // NO terminator and NO trailing newline. Both cases above end the marker's
    // sentence on a `.`, so `end` lands on a real terminator; every fixture in
    // `an_exemption_sentence_with_no_terminator_is_clamped_and_takes_nothing_beyond_it`
    // has a `\n` or a `|` to clamp on.
    //
    // This branch must replace `rest.find('\n').unwrap_or(rest.len())` with
    // "find the terminator and consume it", and the natural shapes of that —
    // `rest.find(is_boundary).map(|i| i + boundary_len).unwrap_or(rest.len() + 1)`,
    // or an `.unwrap()` on the `find` — pass the whole of the rest of this suite
    // and then panic (`byte index N is out of bounds` inside `replace_range`, or
    // an unwrap panic) the first time an owned page of Anvil's ends here. That
    // panic happens inside the review pipeline, on a real PR.
    let (got, remaining_drift) =
        rewrite_anvil_readme("Alpha. DocGuard does **not** yet amend existing documents");
    assert_eq!(
        normalise(&got),
        normalise("Alpha."),
        "a marker sentence that simply runs out of page ends at the end of the \
         page: there is no terminator to consume and none to walk past: {got:?}"
    );
    assert!(remaining_drift.is_empty(), "{remaining_drift:?}");

    // The same file boundary inside a table row that never closes its cell, so
    // the clamp the row would normally supply is absent too.
    let (got, remaining_drift) =
        rewrite_anvil_readme("| Gate | It does **not** yet amend existing documents");
    assert_eq!(
        normalise(&got),
        normalise("| Gate |"),
        "the row's untouched first cell survives; the unterminated, unclamped \
         sentence ends at the end of the page: {got:?}"
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
    let cases: &[(&str, &str)] = &[
        // On separate lines.
        (
            "# Anvil\n\nAlpha. DocGuard does **not** yet amend existing documents. Beta.\n\
             Gamma. Anvil does **not** yet amend existing documents. Delta.\n",
            "# Anvil\n\nAlpha. Beta.\nGamma. Delta.\n",
        ),
        // Two occurrences of the SAME variant on ONE line. The first deletion
        // shifts the second's byte offset, and the first sentence's terminator
        // becomes the second sentence's start boundary. Two implementations
        // that pass every other case here fail this one: a per-line pass
        // (`for line in text.lines() { if line.contains(marker) { delete one
        // sentence } }`) removes only the first and leaves a marker for
        // `remaining_claim` to hard-fail on; and collecting `match_indices`
        // up front, then deleting in ascending order without adjusting for the
        // bytes already removed, slices at stale offsets and can land
        // mid-character.
        (
            "Alpha. X does **not** yet amend existing documents. Beta. Y does **not** yet amend existing documents. Gamma.\n",
            "Alpha. Beta. Gamma.\n",
        ),
        // One of each variant on one line. `EXEMPTION_MARKERS` is a loop over
        // markers, so each pass sees a string the previous pass already edited.
        (
            "Alpha. X does **not** yet amend existing documents. Beta. Y does not yet amend existing documents. Gamma.\n",
            "Alpha. Beta. Gamma.\n",
        ),
    ];

    for (input, expected) in cases {
        let (got, remaining_drift) = rewrite_anvil_readme(input);

        assert!(
            !got.contains(EXEMPTION_MARKER),
            "input {input:?}: every occurrence must be removed, not only the \
             first:\n{got}"
        );
        assert!(
            !got.contains(PLAIN_EXEMPTION_MARKER),
            "input {input:?}: the unbolded variant counts as an occurrence too:\n{got}"
        );
        assert_eq!(
            normalise(&got),
            normalise(expected),
            "input {input:?}: each occurrence takes exactly its own sentence"
        );
        assert_eq!(
            got.lines().count(),
            expected.lines().count(),
            "input {input:?} must keep its line structure, got {got:?}"
        );
        assert!(
            remaining_drift.is_empty(),
            "input {input:?}: a surviving marker is reported as drift and fails the \
             gate: {remaining_drift:?}"
        );
    }
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
//
// STATED EXCLUSION: `doc_files_to_update` comes from a model, and
// `generate_and_write_docs` joins those strings onto `repo_dir` and writes,
// after which the pipeline commits and pushes. What happens for a path that does
// not name a file inside the checkout — `../../evil.md`, an absolute path, or
// the empty string, which joins to `repo_dir` itself — is NOT pinned here. It is
// the same harm shape as #27, the oracle writing files that get pushed onto
// somebody's branch, but it is a different defect from the three this branch
// repairs, and pinning a containment rule here would specify behaviour no issue
// has yet described. Left as a decision for a separate branch, and it should be
// filed as its own issue rather than living only in this comment.

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

    // A blocked PR whose scorecard row says nothing is the same false and
    // unactionable assurance this branch exists to remove — the adverse arm's
    // version of the empty `errored` string forbidden in
    // `a_probe_that_produced_no_judgement_is_errored_and_never_a_pass`, and of
    // the empty summary forbidden on the sufficient arm in
    // `an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply`.
    // `let summary = eval.missing_doc_summary.clone().unwrap_or_default();`
    // satisfies every other #29 case here and blocks this contributor's PR with
    // `is_sufficient: false, errored: None, summary: ""`.
    assert!(
        !report.summary.trim().is_empty(),
        "a gate that fails a pull request must say something a contributor can \
         act on; the probe declining to explain itself is not a licence for the \
         gate to publish an empty reason: {report:?}"
    );

    // And it must still read as an adverse finding rather than as the pass it is
    // not. Pinned as a relation rather than as wording, so this suite does not
    // invent the words: the same probe verdict inverted produces the gate's
    // passing summary, and a blocked pull request may not be described the same
    // way as a passing one.
    let pass_dir = tempdir().unwrap();
    let passed = run_gate(sufficient(), ANVIL, pass_dir.path(), &["src/lib.rs"]);
    assert_ne!(
        report.summary.trim(),
        passed.summary.trim(),
        "the summary of a diff the probe judged under-documented must be \
         distinguishable from the summary of one it judged documented, whether or \
         not the probe said why"
    );
}

#[test]
fn a_probe_that_produced_no_judgement_is_errored_and_never_a_pass() {
    // The headline of this section, and the one the seam existed only to hide
    // until now. A probe that spawned but never ran, exited non-zero, timed
    // out, printed something unparseable, or was abandoned by its watchdog has
    // told the gate NOTHING about this diff. The historical defect is recorded
    // in `evaluate_doc_parity`'s own comment — "This arm previously returned
    // is_doc_sufficient: true, which made gate 1 unfailable" — and the way it
    // comes back is a fix that collapses the match into
    // `.unwrap_or_else(|_| DocParityEvaluation { is_doc_sufficient: true, .. })`
    // while wiring the seam. Nothing else in this suite notices that.
    //
    // The fixture is an empty checkout, so the corpus sync has no owned page to
    // read and nothing of its own to report: an empty file list here means the
    // probe failure produced nothing, and cannot be a rewritten page in
    // disguise.
    for failure in PROBE_FAILURES {
        let dir = tempdir().unwrap();
        let report = run_gate(probe_failed(failure), ANVIL, dir.path(), &["src/lib.rs"]);

        let errored = report.errored.as_deref().unwrap_or_else(|| {
            panic!(
                "{failure:?}: no judgement was obtained, so this is absent evidence \
                 and must be Errored — GateStatus::Errored blocks without claiming \
                 the documentation is deficient. summary was: {}",
                report.summary
            )
        });
        assert!(
            !errored.trim().is_empty(),
            "{failure:?}: an Errored gate that states nothing cannot be acted on"
        );
        assert!(
            !report.is_sufficient,
            "{failure:?}: a probe that produced no judgement cannot have judged the \
             diff documented: {}",
            report.summary
        );
        assert!(
            report.files_created_or_updated.is_empty(),
            "{failure:?}: with no judgement there is no file list to act on, so \
             nothing may be reported as touched: {:?}",
            report.files_created_or_updated
        );
        assert!(
            report.summary.contains(failure),
            "{failure:?}: the gate must state why it could not evaluate parity, so \
             the failure can be told apart from a documentation finding. got: {}",
            report.summary
        );
    }
}

#[test]
fn a_failed_probe_is_not_rescued_by_a_corpus_sync_that_did_have_work_to_do() {
    // The corpus sync runs before the probe and succeeds here, rewriting a real
    // page. Work the gate genuinely did is not evidence about the diff the
    // probe never judged, so it cannot convert absent evidence into a pass.
    let failure = PROBE_FAILURES[0];
    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), &drifting_page());

    let report = run_gate(probe_failed(failure), ANVIL, dir.path(), &["src/lib.rs"]);

    assert!(
        report.errored.is_some(),
        "the probe produced no judgement; a successful sync does not supply one: {}",
        report.summary
    );
    assert!(
        !report.is_sufficient,
        "a rewritten README says nothing about whether the diff is documented: {}",
        report.summary
    );
    assert!(
        report.summary.contains(failure),
        "the gate must still state why parity could not be evaluated: {}",
        report.summary
    );

    // The `Err` arm's fence, and the reason this case carries it: the corpus sync
    // runs BEFORE the probe, so a probe that later failed cannot un-run it. The
    // sync's effect on disk is therefore observable evidence that this report was
    // composed by traversing `ensure_documentation_parity`, not by an override
    // short-circuit at the top of it.
    //
    // Without this, the whole justification for widening `with_probe_override`
    // from `DocParityEvaluation` to `Result<DocParityEvaluation, String>`
    // evaporates: an implementer can satisfy issue #29's headline requirement
    // with `if let Some(Err(e)) = &self.probe_override { return Ok(errored(e)) }`
    // placed before the sync and before the frontmatter loop, leaving the
    // production `Err` path exactly as fragile as it is today while every case in
    // both binaries goes green. The `Ok` arm is already fenced this way — by
    // `the_gate_applies_the_corpus_sync_to_anvils_own_repository`,
    // `both_probe_verdicts_carry_anvils_corpus_sync_outcome_into_the_report`,
    // `the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason`
    // and `an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply` —
    // and this is the only `Err`-arm fixture whose sync has real work to do.
    //
    // It is a behaviour assertion, not a structural one: it says what the page on
    // disk must look like after the gate has run, and says nothing about how the
    // report was assembled.
    let readme = std::fs::read_to_string(dir.path().join("README.md")).unwrap();
    assert!(
        readme.contains(&format!("{TOTAL_GATES}-gate")),
        "the corpus sync runs before the probe, so a probe that later failed \
         cannot un-run it: {readme}"
    );

    // Deliberately free, and stated so it reads as a decision: whether the page
    // the sync really did rewrite is listed in `files_created_or_updated` is not
    // pinned. Listing it is honest (it was written) and omitting it is honest
    // (the gate errored). Both answers are compatible with everything above.
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
    // The token `newly_public` is deliberately ABSENT from this fixture. It used
    // to be present ("Nothing here mentions newly_public."), which made the
    // else-branch's `after.contains("newly_public")` unfalsifiable: the
    // unconditional prose-survival assertion below already required that
    // sentence to survive, so the token was in `after` in every reachable
    // outcome whatever the implementation did. The one assertion written to
    // catch "amended it by appending a generic stub that never mentions the
    // symbol it was named for" could not fail.
    let before = "# Watched\n\nNothing here documents the new public API.\n";
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
        after.contains("Nothing here documents the new public API."),
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
        // The fixture above does not contain `newly_public`, so this is a real
        // requirement and not a restatement of the prose that had to survive.
        // The implementation it kills: "amend" an existing file by appending the
        // generic block `generate_and_write_docs` already writes
        // ("Auto-generated documentation stub by Anvil DocGuard."), then report
        // README.md as updated. The contributor's PR is blocked, the scorecard
        // says README.md was updated to close the gap, and the appended text
        // closes nothing.
        assert!(
            after.contains("newly_public"),
            "README.md was named because `newly_public` is undocumented; an \
             amendment that never mentions it has not closed the gap it was \
             named for, and reporting it as updated is the same false assurance: \
             {after:?}"
        );
    }
}

// =========================================================================
// Issue #29 at the gate that actually decides the merge
// =========================================================================
//
// Everything above pins `DocGuardReport`. `DocGuardReport` is a value; the merge
// decision is not made there. `PreMergeGuard::evaluate_pre_merge_gates` maps the
// report onto gate 1's `GateStatus`, and `PreMergeCertificationReport::seal()`
// sets `is_certified_ready = all_statuses().all(is_acceptable)`.
//
// The mapping, today, is:
//
//     if let Some(err) = &report.errored                  { Errored(err) }
//     else if !report.files_created_or_updated.is_empty() { AutoUpdated }
//     else if report.is_sufficient                        { Passed }
//     else                                                { Failed(summary) }
//
// `is_sufficient` is never consulted once the file list is non-empty, and
// `AutoUpdated.is_acceptable()` is `true`. So the report
// `an_under_documented_diff_does_not_pass_through_the_public_gate` demands —
// `is_sufficient: false`, `errored: None`,
// `files_created_or_updated: ["docs/reference/newly-public.md"]` — certifies the
// pull request as ready. An engineer implements exactly what that case asks for,
// every case in this suite goes green, and gate 1 still passes every
// under-documented diff the probe flagged: DocGuard dutifully wrote a stub, the
// stub makes the file list non-empty, and the evaluator calls that AUTO-SYNCED.
//
// So the mapping is pinned here, in the same branch as the report it consumes.
// `pre_merge_guard::evaluator::doc_parity_status` is the seam: the scaffolding
// extracted the evaluator's inline mapping into a public function, unchanged,
// and `evaluate_pre_merge_gates` calls it. Nothing else in `tests/` pins this —
// `scorecard_wiring_test.rs` is the only other file that touches
// `doc_parity_status`, and only by assigning a `GateStatus` to it directly.
//
// Asserted through `is_acceptable()` and through a sealed report's
// `is_certified_ready`, never against a particular `GateStatus` variant: the
// behaviour under test is the merge decision, not the enum. An implementation
// that adds a variant, or that reports an under-documented diff as `Errored`
// rather than `Failed`, is free to.

/// A `DocGuardReport` with the exact field combination under test.
fn doc_report(
    is_sufficient: bool,
    errored: Option<&str>,
    files: &[&str],
    summary: &str,
) -> DocGuardReport {
    DocGuardReport {
        is_sufficient,
        errored: errored.map(|s| s.to_string()),
        files_created_or_updated: files.iter().map(|f| (*f).to_string()).collect(),
        summary: summary.to_string(),
    }
}

/// Whether a certification report carrying this gate-1 status still certifies.
///
/// Every other gate is `NotMeasured`, which is acceptable, so the verdict is
/// `doc_parity_status`'s alone.
fn certifies_with(status: GateStatus) -> bool {
    let mut report = PreMergeCertificationReport::unmeasured("not evaluated in this fixture");
    report.doc_parity_status = status;
    report.seal();
    report.is_certified_ready
}

/// The gate's stated reason, however the status chooses to carry it.
///
/// Read through `Debug` rather than by matching a variant, so this pins that the
/// reason reaches the certification report at all without dictating which
/// variant carries it or what that variant is called.
fn stated_reason(status: &GateStatus) -> String {
    format!("{status:?}")
}

#[test]
fn a_diff_the_probe_judged_under_documented_does_not_certify_because_a_stub_was_written() {
    // The decisive case, and the live defect: this is the exact report
    // `an_under_documented_diff_does_not_pass_through_the_public_gate` requires
    // DocGuard to produce. The stub DocGuard wrote is a real file and listing it
    // is honest; what is not honest is reading "a file was written" as "the
    // documentation gap is closed". An auto-generated stub carrying the symbol's
    // name in a heading is evidence of the gap, not its repair.
    let status = doc_parity_status(&doc_report(
        false,
        None,
        &["docs/reference/newly-public.md"],
        MISSING_REASON,
    ));

    assert!(
        !status.is_acceptable(),
        "the probe judged this diff under-documented and DocGuard wrote a stub; a \
         stub is not documentation, and gate 1 must not accept it. status: {status:?}"
    );
    assert!(
        !certifies_with(status.clone()),
        "with every other gate unmeasured, this status alone decides the verdict, \
         and an under-documented diff must not certify as ready to merge: {status:?}"
    );
    assert!(
        stated_reason(&status).contains(MISSING_REASON),
        "a blocked pull request must carry the probe's finding to the scorecard; a \
         status that blocks without saying why is unactionable: {status:?}"
    );
}

#[test]
fn a_diff_the_probe_judged_under_documented_does_not_certify_when_no_file_was_written() {
    // The complement, and a regression fence: this arm is correct today
    // (`Failed(summary)`), and the repair for the case above must not break it by
    // keying the decision on the file list from the other side.
    let status = doc_parity_status(&doc_report(false, None, &[], MISSING_REASON));

    assert!(
        !status.is_acceptable(),
        "the probe judged the diff under-documented and nothing was written to \
         change that: {status:?}"
    );
    assert!(
        !certifies_with(status.clone()),
        "an under-documented diff must not certify: {status:?}"
    );
    assert!(
        stated_reason(&status).contains(MISSING_REASON),
        "the finding must reach the certification report: {status:?}"
    );
}

#[test]
fn a_probe_that_produced_no_judgement_does_not_certify() {
    // Absent evidence, at the gate. Correct today, and fenced here for the same
    // reason: the repair above rearranges this chain, and `Errored` is the arm
    // whose historical collapse into a pass made gate 1 unfailable.
    for failure in PROBE_FAILURES {
        let status = doc_parity_status(&doc_report(
            false,
            Some(failure),
            &[],
            "Documentation parity could not be evaluated",
        ));

        assert!(
            !status.is_acceptable(),
            "{failure:?}: no judgement was obtained, and absent evidence is never a \
             pass: {status:?}"
        );
        assert!(
            !certifies_with(status.clone()),
            "{failure:?}: a gate that could not measure must not certify: {status:?}"
        );
        assert!(
            stated_reason(&status).contains(failure),
            "{failure:?}: the reason the gate could not judge must reach the \
             certification report, so it can be told apart from a documentation \
             finding: {status:?}"
        );
    }
}

#[test]
fn an_errored_gate_does_not_certify_even_when_a_page_was_rewritten() {
    // The pairing that produces the wrong answer if the chain is reordered to
    // consult the file list first: the corpus sync really did rewrite Anvil's
    // README, and the probe really did fail. Work done is not evidence about the
    // diff, so the file list must not out-rank `errored`.
    let failure = PROBE_FAILURES[0];
    let status = doc_parity_status(&doc_report(
        false,
        Some(failure),
        &["README.md"],
        "Documentation parity could not be evaluated",
    ));

    assert!(
        !status.is_acceptable(),
        "a rewritten README says nothing about whether the diff is documented: \
         {status:?}"
    );
    assert!(
        !certifies_with(status.clone()),
        "absent evidence must not certify, whatever else the gate got done: {status:?}"
    );
    assert!(
        stated_reason(&status).contains(failure),
        "the probe failure must still be the stated reason: {status:?}"
    );
}

#[test]
fn a_sufficient_diff_certifies_and_a_rewritten_owned_page_does_not_block_it() {
    // The fence on the other side, and the reason it matters: the corpus sync's
    // whole purpose is to repair Anvil's own published counts in the course of
    // certifying a pull request, and gate 1 accepting that is what makes the
    // repair land instead of blocking every Anvil PR that touches a drifted page.
    // A repair for the decisive case above that simply stops accepting a
    // non-empty file list breaks this.
    let rewritten = doc_parity_status(&doc_report(
        true,
        None,
        &["README.md"],
        &format!("Published docs rewritten to TOTAL_GATES={TOTAL_GATES}: README.md"),
    ));
    assert!(
        rewritten.is_acceptable(),
        "the probe judged the diff documented and the sync repaired a page of \
         Anvil's own; that is a pass, not a finding: {rewritten:?}"
    );
    assert!(
        certifies_with(rewritten.clone()),
        "an auto-synced page must not block certification: {rewritten:?}"
    );

    // And the plain pass, so the assertions above are not satisfied by an
    // implementation that has simply stopped distinguishing anything.
    let clean = doc_parity_status(&doc_report(
        true,
        None,
        &[],
        "Documentation and SSOT frontmatters satisfy the required fields and parity rules.",
    ));
    assert!(
        clean.is_acceptable(),
        "nothing adverse was found and nothing needed doing: {clean:?}"
    );
    assert!(
        certifies_with(clean.clone()),
        "a documented diff on a clean corpus certifies: {clean:?}"
    );
}
