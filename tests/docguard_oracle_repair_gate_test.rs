//! Every case that drives `DocGuard::ensure_documentation_parity`, in a binary
//! where `agy` cannot be spawned.
//!
//! These cases used to live in `tests/docguard_oracle_repair_test.rs`, whose
//! header claimed that routing them through `DocGuard::with_probe_override` made
//! an `agy` spawn "structurally unreachable rather than merely unlikely". That
//! claim was false. `Probe` constrains where the probe's outcome is *stored*; it
//! does not oblige `evaluate_doc_parity` to read it. An implementer who wires the
//! override anywhere other than the `Overridden` arm's return — including the
//! entirely plausible `Probe::Overridden(_) => "low".to_string()`, meaning to
//! handle the override elsewhere and forgetting one path — turns those cases into
//! parallel invocations of
//! `agy --print <prompt> --effort low --dangerously-skip-permissions`, each under
//! a 120-second `run_bounded_for` budget, from inside `cargo test`. `agy` is
//! resolved through the inherited `PATH` and it is installed on developer
//! machines, so nothing about that was hypothetical.
//!
//! So the spawn is made unreachable instead of asserted to be: this binary
//! empties `PATH` before any gate call. A fall-through spawn then fails in
//! microseconds with "No such file or directory" rather than invoking a model,
//! the network, or `--dangerously-skip-permissions`, and the assertions below
//! still see exactly what they were written to see. That is the same pattern
//! `tests/docguard_oracle_repair_probe_seam_test.rs` and
//! `tests/docguard_oracle_repair_self_repo_test.rs` already use, and the reason
//! all three are separate binaries: mutating the process environment is a data
//! race against any other thread reading it, and a `cargo test` binary runs its
//! cases in parallel threads.
//!
//! ## Why one `#[test]` and eighteen case functions
//!
//! The environment mutation is only sound if nothing else in this process is
//! running while it happens. Eighteen `#[test]` functions each neutralising
//! `PATH` under a `Once` would still race the first mutation against every other
//! case's `tempdir()` (which reads `TMPDIR`), which is precisely the race the
//! three-binary split exists to avoid. One `#[test]` gives a single-threaded
//! binary and makes the mutation sound by construction.
//!
//! The cost is that the eighteen behaviours share one test name, and it is paid
//! down rather than hidden: each case is a named function with its own doc
//! comment, they are run through `catch_unwind` so **one failure does not mask
//! the others**, every case is reported individually as `ok` or `FAILED`, and the
//! aggregate assertion prints every failure with the case name that produced it.
//! A run of this binary therefore tells you exactly which behaviours are red,
//! which is the property that matters for red evidence.
//!
//! ## What is pinned here
//!
//! Issue #27's gate-level requirements (the skipped sync is stated in the gate's
//! summary; Anvil's own sync still runs; a corpus that is not Anvil's is never
//! opened, so it can never *fail* the gate either) and issue #29's requirements
//! on `DocGuardReport` (absent or failed evidence is never a pass). The
//! pure-function cases — the corpus sync called directly, the exemption
//! rewriter, and the `DocGuardReport -> GateStatus` mapping, which is where
//! issue #29's *merge* decision is actually made — stay in
//! `tests/docguard_oracle_repair_test.rs`, because none of them can reach the
//! probe at all.
//!
//! ## Six of the eighteen cases here are FENCES, not red evidence
//!
//! Seventeen of the eighteen are currently blocked at a seam `todo!()`
//! (`src/doc_guard/mod.rs:172` and `:228`), so an unmodified run reports 17
//! failing and one passing. That number must not be read as seventeen
//! behaviours failing against the three live defects, and this file used to let
//! it be read that way.
//!
//! Measured, not assumed: with ONLY the four scaffolding bodies filled in
//! (`with_probe_override` -> `Self { probe: Probe::Overridden(outcome) }`;
//! `with_probe_output_override` -> `Self { probe: Probe::SuppliedOutput(output) }`;
//! the `Probe::Overridden` arm in `evaluate_doc_parity` ->
//! `return outcome.clone().map_err(anyhow::Error::msg)`; the
//! `Probe::SuppliedOutput` arm -> `return classify_probe_output(..)`) and
//! nothing else touched — no defect repaired — this binary reports **12 failing
//! on test-file assertion lines and these six passing**:
//!
//! * `a_probe_that_produced_no_judgement_is_errored_and_never_a_pass`
//! * `a_failed_probe_is_not_rescued_by_a_corpus_sync_that_did_have_work_to_do`
//! * `a_corpus_sync_that_could_not_run_at_all_is_errored_at_the_gate`
//! * `published_drift_the_sync_could_not_repair_fails_anvils_own_gate`
//! * `a_finding_the_gate_reached_before_the_probe_is_the_finding_it_reports`
//! * `a_live_probe_that_could_not_run_is_errored_through_the_real_call_path`
//!   (which needs no seam at all, and so is the one case that passes on an
//!   unmodified tree)
//!
//! They are regression fences on arms of `ensure_documentation_parity` that are
//! already CORRECT on `main` — the same category, and the same disclosure, that
//! `tests/docguard_oracle_repair_test.rs` gives its four green mapping cases at
//! the head of that file. Each names the arm it fences and the repair that could
//! break it:
//!
//! * The first two fence the `Err` arm of the `evaluate_doc_parity` match: a
//!   probe that produced no judgement returns `errored: Some(reason)`,
//!   `is_sufficient: false`, an empty file list, and a summary carrying the
//!   reason. The repair that breaks it is reordering the tail of
//!   `ensure_documentation_parity` so work the gate got done — a stub written by
//!   `generate_and_write_docs`, or a page the corpus sync rewrote — out-ranks
//!   `errored` and is reported as a non-empty `files_created_or_updated` on a run
//!   that has no judgement. The second case is the one that carries a corpus sync
//!   with real work to do, which is exactly the pairing that produces the wrong
//!   answer under that reorder.
//! * The third fences the `Err` arm of the corpus-sync match. The repair that
//!   breaks it is collapsing that match to
//!   `let sync = sync_published_counts(..)?;` while threading `not_applicable`
//!   through it — which propagates the error out of the gate instead of mapping
//!   it onto `errored`, or, with the `?` swallowed, reports a corpus the gate
//!   could not read as sufficient.
//! * The fourth fences the DRIFT arm of that same match: published claims the
//!   sync rewrote the page for and still could not make honest fail the gate,
//!   with an empty `files_created_or_updated`. It is the sibling of the third
//!   and it falls to the same flattening refactor — and it is the more dangerous
//!   loss of the two, because `rewritten` is non-empty on its fixture, so the
//!   arm's disappearance does not merely stop failing the pull request, it
//!   announces the unrepaired page as a completed documentation update and
//!   certifies. See `unrepairable_drift_page()` for why a fixture that reaches
//!   this arm has to be built rather than merely written down.
//! * The fifth is a fence of a different kind: it fences the ORDER of the gate's
//!   own steps — the frontmatter check runs before the probe, so a diff that
//!   violates it is reported as that finding and the probe's outcome is not
//!   observable in the report at all. `main` already behaves this way, so it is
//!   green from the moment the seam compiles. It is here because the two `Err`
//!   fences above close the "override consulted too early" hole in only ONE of
//!   its two placements: the README-on-disk assertion in the second of them
//!   proves the override was not read BEFORE the corpus sync, and nothing
//!   proved it was not read AFTER the sync but BEFORE the frontmatter loop —
//!   which is the placement this header claims to have killed. The repair that
//!   breaks it is an early `if let Probe::Overridden(Err(e)) = &self.probe`
//!   return sitting between the corpus-sync match and the frontmatter loop.
//!   MEASURED, not argued: with that mutant applied on top of the seam
//!   scaffolding this binary reports one more failing case than without it, and
//!   the one case that flips is this one — every other `Err`-arm assertion in
//!   all four binaries is satisfied by the mutant. What the mutant costs is
//!   that no test in any binary reaches the real `Err(e) =>` arm at the
//!   `evaluate_doc_parity` call site, which is the arm whose historical collapse
//!   into `is_doc_sufficient: true` made gate 1 unfailable.
//! * The sixth is
//!   `a_live_probe_that_could_not_run_is_errored_through_the_real_call_path`,
//!   and it fences something no other case in any binary touches: the
//!   PRODUCTION of a probe failure, as opposed to its consumption. Every case
//!   above hands the gate a failure string a test wrote. This one hands it
//!   nothing, lets the real probe closure run with an empty `PATH`, and reads
//!   the failure back out of the report — so `run_bounded_for`,
//!   `run_with_watchdog` and the watchdog's own fallback are all traversed by
//!   the product rather than described by a comment.
//!
//! Nothing about them needs to change; they are correctly aimed and correctly
//! falsifiable. What was wrong was publishing "16/16 red" as behavioural red
//! evidence when several of the cases prove only that a seam is unimplemented
//! and then go green. On a branch whose subject is ADR-0002's honesty law, a
//! published number that does not match the measurement is that same defect one
//! level up.
//!
//! ## One more thing the failing count does not say, and should
//!
//! Of those failing cases, FOUR die inside the `skipped_sync_reason()` helper rather
//! than on the assertion the case is named for —
//! `the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason`,
//! `the_gate_applies_the_corpus_sync_to_anvils_own_repository`,
//! `both_probe_verdicts_carry_anvils_corpus_sync_outcome_into_the_report` and
//! `an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply`, all
//! four reporting `"oyatie/console" is not Anvil's repository, so the sync did
//! not apply and must say so before any caller can repeat it`.
//!
//! That is still a specified behaviour failing — issue #27's `not_applicable` —
//! and once ownership lands the four proceed to the assertions they are named
//! for. But it means the third of them,
//! `the_gate_applies_the_corpus_sync_to_anvils_own_repository`, is effectively a
//! further fence at review time: its own subject (Anvil's sync still runs at the
//! gate) is correct on `main`, and its redness today comes from the helper.
//! Recorded here so the number is read for what it measures.
//!
//! ## The two cases at the end of this file, and what they close
//!
//! `PROBE_FAILURES` is five strings a test wrote, and every case that uses them
//! pins what the gate DOES with a probe failure. Until now nothing in any binary
//! ran the code that decides whether a probe run IS a failure, which is the one
//! arm this whole branch keeps naming as the one whose collapse made gate 1
//! unfailable. Two cases close that, neither of them spawning anything:
//!
//! * `a_live_probe_that_could_not_run_is_errored_through_the_real_call_path`
//!   runs the real probe path with an empty `PATH`, so `run_bounded_for` fails
//!   to resolve `agy` before any process exists and the failure the gate reports
//!   is one the product produced.
//! * `the_supplied_probe_output_is_classified_by_the_exported_classifier`
//!   supplies a completed probe RUN — exit status, stdout, stderr — through
//!   `DocGuard::with_probe_output_override`, and requires the gate's report to
//!   agree with what `doc_guard::classify_probe_output` returns for that same
//!   run. `classify_probe_output`'s own behaviour is pinned directly in
//!   `tests/docguard_oracle_repair_test.rs`; this is the binding that stops a
//!   second, private copy from deciding it while the exported one stays
//!   correctly repaired and uncalled.

use anvil::doc_guard::corpus_sync::sync_published_counts;
use anvil::doc_guard::{
    DocGuard, DocGuardReport, DocParityEvaluation, FrontmatterValidator, classify_probe_output,
};
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use tempfile::tempdir;

/// Anvil's own repository: the only one whose published gate counts are
/// `TOTAL_GATES`. See the constant of the same name in
/// `tests/docguard_oracle_repair_test.rs` for why ownership is a compile-time
/// constant rather than an environment lookup.
const ANVIL: &str = "oyatie/anvil";

/// Every page `collect_owned_pages` claims, so a partial skip is caught too.
const OWNED_PAGES: &[&str] = &[
    "README.md",
    "docs/doctrine.md",
    "openapi/openapi.yaml",
    "docs/adr/0001-console.md",
    "docs/decisions/0001-console.md",
];

/// Pages in Anvil's own checkout that the corpus deliberately does **not** own.
/// See the constant of the same name in `tests/docguard_oracle_repair_test.rs`
/// for the two wrong implementations these exist to kill, and for why each path
/// is the one it is.
///
/// In short: a sync that WALKS the checkout instead of enumerating the corpus
/// reaches every owned page, passes every one-directional assertion in all four
/// binaries, and rewrites Anvil's CHANGELOG and `docs/` notes on every one of
/// its own pull requests — that is what `CHANGELOG.md` and
/// `docs/notes/roadmap.md` kill. A sync that SHALLOW-GLOBS the directories the
/// corpus already lives in (`*.md` directly under `docs/`, plus the two ADR
/// directories) survives both of those, because neither is enumerated by it —
/// and `docs/runbook.md`, a sibling of `docs/doctrine.md` at the same depth in
/// the same directory, is what kills that one.
const NOT_OWNED_PAGES: &[&str] = &["CHANGELOG.md", "docs/notes/roadmap.md", "docs/runbook.md"];

/// The reason a probe gives for judging a diff under-documented. Held as a
/// constant so the assertions that it reaches `DocGuardReport::summary` pin
/// pass-through rather than wording.
const MISSING_REASON: &str = "newly_public is a new public API with no reference page";

/// The five ways `evaluate_doc_parity` can come back with no judgement at all.
/// The strings are the shapes the real call site produces (spawn failure,
/// non-zero exit, timeout, unparseable output, watchdog supervision failure);
/// their wording is the test's, and it is only ever asserted as pass-through.
///
/// These strings therefore say nothing about whether the product produces `Err`
/// in those five situations — they are the CONSUMPTION side only. The production
/// side is pinned separately and against the product's own output, by the two
/// cases at the end of this file and by the `classify_probe_output` /
/// `probe_supervision_failure` section of
/// `tests/docguard_oracle_repair_test.rs`.
const PROBE_FAILURES: &[&str] = &[
    "failed to run doc parity probe: No such file or directory (os error 2)",
    "doc parity probe exited with status exit status: 1: permission check failed for command",
    "doc parity probe timed out after 120s",
    "doc parity probe returned no parseable evaluation (stdout 4096 bytes)",
    "doc parity probe supervision failed: watchdog channel closed",
];

/// Mirrors `drifting_page()` in the main suite: an Anvil page publishing claims
/// that are deliberately *not* `TOTAL_GATES`, in both the digit form and the
/// spelled-out `sixty-gate` form.
fn drifting_page() -> String {
    format!(
        "# Anvil\n\
         \n\
         The fabric ships behind a {}-gate release check.\n\
         It replaced the sixty-gate pilot programme.\n",
        TOTAL_GATES + 1
    )
}

/// Mirrors `drifting_page_with_exemption()` in the main suite: the same drift
/// plus an exemption sentence, so one fixture triggers all three of
/// `rewrite_page`'s mutations.
///
/// Used for BOTH the owned and the not-owned pages in
/// `the_gate_applies_the_corpus_sync_to_anvils_own_repository`, so the two sets
/// are byte-identical in content and the only thing that can separate them is
/// the corpus boundary itself.
fn drifting_page_with_exemption() -> String {
    format!(
        "# Anvil\n\
         \n\
         The fabric ships behind a {}-gate release check.\n\
         It replaced the sixty-gate pilot programme.\n\
         Roadmap. DocGuard does **not** yet amend existing documents such as `README.md`. Support is planned.\n",
        TOTAL_GATES + 1
    )
}

/// Mirrors `watched_repo_page()` in the main suite: a page belonging to a
/// repository that is **not** Anvil's, carrying all three mutations
/// `rewrite_page` performs, so `assert_eq!(got, page)` means *no owned page in
/// that repository is modified*.
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

/// Mirrors `already_honest_page()` in the main suite: a page with nothing at all
/// for the sync to do, so an applied sync and a skipped sync have exactly the
/// same work to report on it — none.
fn already_honest_page() -> String {
    format!("# Page\n\nShips behind a {TOTAL_GATES}-gate release check.\n")
}

/// An Anvil page the corpus sync writes and still cannot vouch for.
///
/// Building one takes care, and the care is the point. `rewrite_page` and
/// `remaining_claim` share `count_regex` and `sixty_regex`, so every claim the
/// checker can see the rewriter has already normalised: no page can simply
/// *arrive* carrying drift that survives its own repair. A previous round of
/// this suite concluded from that that the drift arm was unreachable and left it
/// unpinned. It is not unreachable — the drift a correct sync is left holding is
/// drift its own edit CREATES.
///
/// Here the exemption sentence occupies a whole line between `Anvil ships 12`
/// and `-gate release check.`. Deleting it closes the gap, and `count_regex`'s
/// `\s*-\s*gate` spans the newline, so the repaired page publishes a `12`-gate
/// claim that was not there before and that nothing will rewrite. `12` is not
/// `TOTAL_GATES`, so the sync reports it as remaining drift.
///
/// The shape is deliberately independent of HOW issue #28's deletion is
/// repaired. Whether the deletion consumes the marker's trailing newline, leaves
/// the line blank, or leaves a space behind, `\s*` spans all three and the joined
/// claim still reads `12`.
///
/// It is NOT independent of WHEN it is repaired, and that dependency is a stated
/// rule rather than an assumption about today's code. The rule block at the head
/// of the issue-#28 section in `tests/docguard_oracle_repair_test.rs` requires
/// the gate-count and `sixty-gate` rewrites to run BEFORE the exemption-sentence
/// deletion, and forbids re-running them over the spliced text afterwards —
/// because a claim manufactured at the deletion's junction is a number nobody
/// authored, and publishing it silently repaired is the class of statement this
/// branch exists to stop the oracle making. This fixture is that rule's witness:
/// the `12` and the `-gate` are only brought together BY the deletion, so a
/// count pass that ran first cannot see them, and `remaining_claim` reports the
/// joined claim as drift.
///
/// An implementation that deletes first and then normalises leaves
/// `remaining_drift` empty here, and `unrepaired_drift_entry()`'s fence panics.
/// That panic means "the implementation broke the stated ordering rule", not
/// "rebuild the fixture" — its message says so. And the case does not assume the
/// fixture works: it asserts the drift at the sync before it asserts anything
/// about the gate, so a rewriter that did normalise this fails loudly instead of
/// letting the gate assertions pass vacuously.
fn unrepairable_drift_page() -> String {
    "# Anvil\n\
     \n\
     Anvil ships 12\n\
     It does **not** yet amend existing documents.\n\
     -gate release check.\n"
        .to_string()
}

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
}

/// Builds `docs/adr` with one ADR in it and then makes the directory itself
/// unreadable, so the sync's **first** filesystem read fails: not the per-page
/// `read_to_string` loop, but `collect_owned_pages`'s `read_dir`, which runs
/// before it.
///
/// Returns the directory's original permissions so the caller can restore them
/// (an unreadable directory defeats `TempDir`'s cleanup), or `None` when this
/// process is privileged enough to read a `0o000` directory — under a root
/// container the fixture cannot be built at all, and the caller says so rather
/// than passing vacuously.
///
/// STATED COST: this fixture is unix-only, and under a root CI container it
/// fails with a diagnostic instead of running. Both are recorded rather than
/// worked around, because the alternative — skipping quietly — would leave the
/// read path looking fenced when it was not tested at all.
#[cfg(unix)]
fn make_adr_dir_unreadable(repo_dir: &Path) -> Option<std::fs::Permissions> {
    use std::os::unix::fs::PermissionsExt;

    let adr = repo_dir.join("docs/adr");
    std::fs::create_dir_all(&adr).unwrap();
    std::fs::write(adr.join("0001-x.md"), "# ADR\n").unwrap();

    let original = std::fs::metadata(&adr).unwrap().permissions();
    std::fs::set_permissions(&adr, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&adr).is_ok() {
        std::fs::set_permissions(&adr, original).unwrap();
        return None;
    }
    Some(original)
}

/// Restores `docs/adr`'s permissions on drop, so a panic *between* the fixture
/// being built and the assertions running cannot leave a `0o000` directory
/// behind. `TempDir`'s own cleanup cannot remove one, so every such panic
/// otherwise leaks an undeletable directory into `TMPDIR` — and for the whole of
/// the red phase, `run_gate` panics on the seam's `todo!()` before the plain
/// `restore_adr_dir` call could be reached.
#[cfg(unix)]
struct RestoreAdrDir<'a> {
    repo_dir: &'a Path,
    original: Option<std::fs::Permissions>,
}

#[cfg(unix)]
impl Drop for RestoreAdrDir<'_> {
    fn drop(&mut self) {
        if let Some(original) = self.original.take() {
            let _ = std::fs::set_permissions(self.repo_dir.join("docs/adr"), original);
        }
    }
}

/// The reason the sync gives for declining to apply to a repository that is not
/// Anvil's, read back from the sync itself so every assertion about it pins
/// pass-through rather than wording.
///
/// STATED REQUIREMENT: the reason is a property of *which repository is under
/// review*, not of what happens to be on disk. It is derived here in a tempdir
/// other than the one the gate under test runs in, so a reason that enumerated
/// the pages it declined to touch, or that named their paths, would not satisfy
/// this suite.
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

/// The drift entry the sync reports for `unrepairable_drift_page()`, read back
/// from the sync itself so the assertion that it reaches the gate's summary pins
/// pass-through rather than wording.
///
/// STATED REQUIREMENT: a drift entry names the owned page *relative to the
/// checkout* and the claim that page still makes. It is derived here in a
/// tempdir other than the one the gate under test runs in, so an entry that
/// carried an absolute path would not satisfy this suite — which is the right
/// requirement independently, since this string is published into a pull
/// request's scorecard and a reviewer's machine paths do not belong there.
fn unrepaired_drift_entry() -> String {
    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), &unrepairable_drift_page());
    let sync = sync_published_counts(ANVIL, dir.path(), TOTAL_GATES)
        .expect("the fixture is readable, so the sync itself must succeed");
    assert_eq!(
        sync.remaining_drift.len(),
        1,
        "fence: `unrepairable_drift_page()` exists to leave exactly one claim the \
         sync wrote and cannot vouch for.\n\
         If this is EMPTY, the rewriter normalised a claim that only exists \
         because the exemption deletion spliced `12` onto `-gate` — which means \
         the count pass ran AFTER the deletion. That is a violation of the stated \
         ordering rule at the head of the issue-#28 section in \
         `tests/docguard_oracle_repair_test.rs`, not a stale fixture: a number \
         nobody authored was manufactured at the deletion's junction and then \
         published as repaired. Fix the ordering; do not rebuild the fixture \
         around it.\n\
         If this has MORE than one entry the fixture has drifted and should be \
         rebuilt — but it must be rebuilt rather than deleted, because the gate \
         arm it drives is live. got: {:?}",
        sync.remaining_drift
    );
    assert_eq!(
        sync.rewritten,
        vec!["README.md".to_string()],
        "fence: the sync must actually have WRITTEN this page — that is what makes \
         the case discriminating, because the gate must report the page it wrote \
         as no work done"
    );
    sync.remaining_drift.into_iter().next().unwrap()
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
        // openapi/openapi.yaml and docs/adr/*.md of the repository under test.
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

/// A completed process' exit status, built rather than obtained, so a probe run
/// of a given shape can be handed to the gate without any process existing.
///
/// STATED COST: there is no portable constructor, so on a non-unix target this
/// fixture cannot be built. It panics there rather than letting the case that
/// needs it disappear from the run.
#[cfg(unix)]
fn exit_status(code: i32) -> std::process::ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code << 8)
}

#[cfg(not(unix))]
fn exit_status(_code: i32) -> std::process::ExitStatus {
    panic!(
        "this fixture needs a constructed `ExitStatus`, which only the unix \
         extension trait provides. The behaviour is not satisfied on this \
         platform, it is unmeasured on it"
    )
}

/// A completed probe run: what `run_bounded_for` hands the classification.
fn probe_output(code: i32, stdout: &str, stderr: &str) -> std::process::Output {
    std::process::Output {
        status: exit_status(code),
        stdout: stdout.as_bytes().to_vec(),
        stderr: stderr.as_bytes().to_vec(),
    }
}

/// A judgement of sufficiency, printed the way the probe's own prompt asks for
/// it. Held as stdout rather than as a `DocParityEvaluation` because these two
/// constants exist to be PARSED by the code under test.
const PRINTED_SUFFICIENT: &str = "```json\n\
     {\"is_doc_sufficient\": true, \"missing_doc_summary\": null, \
     \"doc_files_to_update\": [], \"suggested_adr_title\": null}\n\
     ```\n";

/// A judgement of insufficiency, naming one file, printed the same way.
const PRINTED_INSUFFICIENT: &str = "```json\n\
     {\"is_doc_sufficient\": false, \"missing_doc_summary\": \
     \"newly_public is a new public API with no reference page\", \
     \"doc_files_to_update\": [\"docs/reference/newly-public.md\"], \
     \"suggested_adr_title\": null}\n\
     ```\n";

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
// Issue #27 at the gate — the sync is scoped, and the skip is stated
// =========================================================================

/// Reviewing somebody else's repository must not edit a single page of it, and
/// the edit is the harm because the pipeline commits and pushes it onto the
/// contributor's branch.
fn reviewing_a_repository_that_is_not_anvil_leaves_its_owned_pages_byte_identical() {
    // Driven through the probe seam, which replaces only the doc-parity probe:
    // the corpus-sync call this case is actually about is still the real one.
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

/// A finding the gate reached on its own, before any probe ran, is the finding
/// it reports — and the probe's outcome is not observable in it at all.
///
/// # Why this case exists
///
/// The `Err`-arm cases in this binary
/// (`a_probe_that_produced_no_judgement_is_errored_and_never_a_pass`,
/// `a_failed_probe_is_not_rescued_by_a_corpus_sync_that_did_have_work_to_do`)
/// close the "override consulted too early" hole in only ONE of its two
/// placements. The README-on-disk assertion in the second of those proves the
/// override was not consulted BEFORE the corpus sync. Nothing in this binary
/// proved it was not consulted AFTER the sync and BEFORE the frontmatter loop —
/// and that second placement is exactly the one this file's header claims to
/// have killed.
///
/// The wrong implementation, sitting in `ensure_documentation_parity`
/// immediately after the corpus-sync match and before the
/// `for file in &diff_ctx.changed_files` loop:
///
/// ```ignore
/// if let Probe::Overridden(Err(e)) = &self.probe {
///     return Ok(DocGuardReport {
///         errored: Some(e.clone()),
///         is_sufficient: false,
///         files_created_or_updated: Vec::new(),
///         summary: format!("Documentation parity could not be evaluated: {e}"),
///     });
/// }
/// ```
///
/// Every `Err`-arm assertion in all four binaries is satisfied by it. The sync
/// has already run, so `README.md` carries its repaired `TOTAL_GATES` claim;
/// `errored` is `Some` and non-empty; `is_sufficient` is false; the summary
/// carries the failure string; the file list is empty; the probe-seam binary's
/// absolute `seam-sentinel:` check passes on both runs and the two runs agree.
/// `reviewing_a_repository_that_is_not_anvil_leaves_its_owned_pages_byte_identical`
/// does not catch it either, because that case supplies `sufficient()`, so the
/// early return is never taken there.
///
/// What it costs is the whole point. Under that wiring NO test that supplies a
/// probe OUTCOME reaches the real `Err(e) =>` arm at the
/// `self.evaluate_doc_parity(..)` call site. That arm is precisely the one whose
/// historical collapse into `is_doc_sufficient: true` made gate 1 unfailable —
/// the comment recording it is still in the source. An implementer who, while
/// wiring the seam, reverts it to
/// `Ok(DocParityEvaluation { is_doc_sufficient: true, .. })` would ship an
/// unfailable gate 1 with every `PROBE_FAILURES` case in this binary green,
/// because none of the five traverses it.
///
/// CORRECTED, and the correction matters: an earlier version of this paragraph
/// added "nor `evaluate_doc_parity`'s own watchdog fallback", and went on to
/// treat that as something the mutant alone cost. It is not the mutant's doing.
/// The ACCEPTED design has the override answer at the point the judgement is
/// produced, which is before `run_with_watchdog` is entered at all, so no case
/// that supplies an outcome was ever going to reach the watchdog or its
/// fallback. Publishing that as a property the suite had was the same defect
/// this branch exists to remove, one level up. It is now a property the suite
/// really has, and it is
/// `a_live_probe_that_could_not_run_is_errored_through_the_real_call_path` at
/// the end of this file that gives it: nothing supplied, nothing spawned, the
/// real closure and the real supervision run, and the failure the gate reports
/// is one the product produced.
///
/// # What is asserted, and why it is behaviour rather than structure
///
/// The frontmatter check runs before the probe, so on a diff that violates it
/// the gate has a finding of its own and never needs a judgement. The report
/// must therefore BE that finding: not errored, adverse, carrying the
/// validator's own message — and carrying no trace of the supplied probe
/// outcome, because a probe that was never consulted cannot have said anything.
///
/// That is a statement about what the report IS, not about how it was built or
/// where the override is read. It happens to be unsatisfiable by any wiring that
/// consults the override earlier than the point the real probe runs, which is
/// the requirement `with_probe_override`'s contract already states in prose and
/// which nothing until now measured.
fn a_finding_the_gate_reached_before_the_probe_is_the_finding_it_reports() {
    // Anvil's OWN repository, so the corpus sync genuinely applies and the case
    // cannot be satisfied by an ownership skip: every path this report could
    // have taken is live.
    let dir = tempdir().unwrap();
    for owned in OWNED_PAGES {
        write(&dir.path().join(owned), &already_honest_page());
    }

    // `already_honest_page()` above is deliberate: the sync has nothing to
    // repair on it, so neither the drift arm nor the `Err` arm of the
    // corpus-sync match can fire and the run reaches the frontmatter loop. The
    // fence says so, rather than leaving it to be inferred.
    let sync = sync_published_counts(ANVIL, dir.path(), TOTAL_GATES)
        .expect("fence: the fixture is readable, so the sync must succeed");
    assert!(
        sync.remaining_drift.is_empty(),
        "fence: this case is about the frontmatter finding, so the corpus sync \
         must not return first: {:?}",
        sync.remaining_drift
    );

    let policy = "---\nstatus: active\ncanonical_authority: true\n---\n\n# Tenancy\n";
    write(&dir.path().join("tenancy/policy.md"), policy);

    // The validator's message is read from the validator, not written down here,
    // so the assertion that it reaches the summary pins pass-through rather than
    // wording — and doubles as the fence that the fixture still takes the
    // frontmatter early-return path at all.
    let violation =
        FrontmatterValidator::validate_doc_frontmatter("tenancy/policy.md", policy, dir.path())
            .expect_err(
                "this case pins the report composed on the frontmatter early-return \
                 path; the fixture no longer takes it",
            );

    // Every one of the five, not just the first: an implementation that reads
    // the override early cannot be excused by the particular failure string it
    // happened to be handed.
    for failure in PROBE_FAILURES {
        let report = run_gate(
            probe_failed(failure),
            ANVIL,
            dir.path(),
            &["tenancy/policy.md"],
        );

        assert!(
            report.errored.is_none(),
            "{failure:?}: the gate found a frontmatter violation on its own, before \
             any probe was needed. That is a judgement, not absent evidence, and \
             reporting it as Errored means the supplied probe outcome was consulted \
             on a run that never had to ask for one: {:?}",
            report.errored
        );
        assert!(
            !report.is_sufficient,
            "{failure:?}: the frontmatter violation is a real adverse finding: {}",
            report.summary
        );
        assert!(
            report.summary.contains(&violation),
            "{failure:?}: the report must be the FRONTMATTER finding, so it must \
             carry the validator's own message {violation:?}. A summary that says \
             anything else is a report about a probe this run had no reason to \
             consult: {}",
            report.summary
        );
        assert!(
            !report.summary.contains(*failure),
            "{failure:?}: the frontmatter check returned before the probe ran, so \
             the supplied probe outcome must not be observable in the report at \
             all. Seeing it here means the override is read somewhere other than \
             the point the real probe produces its judgement — which leaves the \
             real `Err` arm at the `evaluate_doc_parity` call site, the arm whose \
             collapse into `is_doc_sufficient: true` made gate 1 unfailable, \
             untraversed by every case in this binary: {}",
            report.summary
        );
    }

    // Stated exclusion, so it is a decision rather than an omission:
    // `files_created_or_updated` is NOT asserted here. The corpus sync had no
    // work to do on this fixture, so whether the frontmatter early return
    // reports the sync's (empty) rewrite list or an empty list of its own is not
    // a distinction this fixture can draw, and inventing one would pin a
    // composition detail rather than a behaviour. The requirement that an
    // adverse finding never announces work it did not do is carried by
    // `an_under_documented_diff_that_named_no_files_still_fails_the_gate` and by
    // the two write-failure cases.
}

/// A skipped sync must be *stated* at the gate, on both probe verdicts, and it
/// must never turn into absent evidence — including when the corpus it declined
/// to open could not have been read anyway.
fn the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason() {
    // Both probe verdicts are exercised: an implementation that appends the
    // reason only inside the `is_doc_sufficient` branch leaves a non-Anvil
    // repository with an under-documented diff reading as though the sync had
    // applied.
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

    // The fail-closed mirror, pinned at the GATE rather than at the sync,
    // because that is where the harm lands: `ensure_documentation_parity` maps
    // an `Err` from the sync onto `errored`, gate 1 goes Errored, and
    // `GateStatus::Errored` is not acceptable — so every pull request on this
    // repository is blocked by a corpus that is not Anvil's, was never Anvil's
    // business, and that Anvil had no reason to open.
    //
    // Two fixtures, because `sync_published_counts` performs TWO filesystem
    // reads and the ownership decision has to precede BOTH:
    //
    //   1. `README.md` is a directory — the per-page `read_to_string` loop
    //      fails.
    //   2. `docs/adr` exists, is non-empty, and is unreadable —
    //      `collect_owned_pages`'s `read_dir` fails, which happens BEFORE the
    //      page loop.
    //
    // The second is the one that separates the right guard from the wrong one:
    // the natural place to reach for the ownership check is one line too low,
    // just after `let pages = collect_owned_pages(repo_dir)?;`, because that is
    // where you already are. See the matching sync-level pair in
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

    #[cfg(unix)]
    {
        let dir = tempdir().unwrap();
        let Some(original) = make_adr_dir_unreadable(dir.path()) else {
            panic!(
                "fixture: this process can read a 0o000 directory, so the \
                 unreadable-ADR fixture cannot be built. That is a root container, \
                 not a passing implementation — run this suite as a non-root user. \
                 (The `README.md`-is-a-directory fixture above still ran, but it \
                 only fences the per-page read, not `collect_owned_pages`.)"
            );
        };
        let _restore = RestoreAdrDir {
            repo_dir: dir.path(),
            original: Some(original),
        };
        let anvil_result = sync_published_counts(ANVIL, dir.path(), TOTAL_GATES);
        let report = run_gate(sufficient(), "oyatie/console", dir.path(), &["src/lib.rs"]);
        drop(_restore);

        assert!(
            anvil_result.is_err(),
            "fence: an unreadable `docs/adr` must remain a real failure for Anvil's \
             own corpus, or the assertions below are not about a corpus the sync \
             could not read"
        );
        assert!(
            report.errored.is_none(),
            "oyatie/console: `docs/adr` in somebody else's checkout is not Anvil's \
             corpus, and listing it is not something this gate had any reason to do. \
             An ownership check placed after `collect_owned_pages` blocks every PR on \
             this repository at gate 1 on a directory Anvil should never have \
             opened: {:?}",
            report.errored
        );
        assert!(
            report.is_sufficient,
            "oyatie/console: the probe judged the diff documented and the skipped \
             sync has no finding of its own to add: {}",
            report.summary
        );
        assert!(
            report.summary.contains(&reason),
            "oyatie/console: the skip must still be stated on this path too: {}",
            report.summary
        );
        assert!(
            report.files_created_or_updated.is_empty(),
            "oyatie/console: nothing was rewritten, so nothing may be reported as \
             touched: {:?}",
            report.files_created_or_updated
        );
    }
}

/// The mirror of the scoping cases: Anvil's own repository still gets synced by
/// the gate itself.
fn the_gate_applies_the_corpus_sync_to_anvils_own_repository() {
    // Every other #27 case calls `sync_published_counts` directly, so nothing
    // would oblige `ensure_documentation_parity` to keep calling it at all.
    // Deleting that call, or putting it behind a condition the gate never
    // satisfies, is the cheapest way to make the scoping cases green — and it
    // would silently remove Anvil's own published-count enforcement from gate 1.
    //
    // Every entry of `OWNED_PAGES` carries the drift, not just `README.md`.
    // `OWNED_PAGES` used to appear in this binary only in the NEGATIVE direction,
    // against repositories that are not Anvil's, while every Anvil fixture wrote
    // one page — so narrowing what the sync rewrites while restructuring it for
    // issue #27 (`if rel != "README.md" { continue; }`, or a `.md`-only
    // exemption deletion) left Anvil's own doctrine, OpenAPI document and ADRs
    // publishing stale counts with gate 1 reporting the corpus clean.
    //
    // And the counter-pressure, in the same run and on the same bytes:
    // `NOT_OWNED_PAGES` carries the identical drifting page. Without it this case
    // pushes one way only — "reach every owned page" — and a sync that walks the
    // checkout rather than enumerating the corpus satisfies that while rewriting
    // Anvil's CHANGELOG and `docs/` notes and reporting them as documentation
    // updates the pipeline then commits and pushes.
    let page = drifting_page_with_exemption();
    let dir = tempdir().unwrap();
    for owned in OWNED_PAGES {
        write(&dir.path().join(owned), &page);
    }
    for unowned in NOT_OWNED_PAGES {
        write(&dir.path().join(unowned), &page);
    }

    let report = run_gate(sufficient(), ANVIL, dir.path(), &["src/lib.rs"]);

    for owned in OWNED_PAGES {
        let got = std::fs::read_to_string(dir.path().join(owned)).unwrap();
        assert!(
            got.contains(&format!("{TOTAL_GATES}-gate")),
            "the gate itself must apply the sync to Anvil's own {owned}: {got}"
        );
        assert!(
            !got.contains("does **not** yet amend existing documents"),
            "the exemption marker must go from every owned page of Anvil's, not \
             only from the one the fixtures happened to use: {got}"
        );
        assert!(
            !got.contains(&format!("{}-gate", TOTAL_GATES + 1)),
            "the drifting claim must be gone from Anvil's own {owned}: {got}"
        );
        assert!(
            !got.to_lowercase().contains("sixty-gate"),
            "the spelled-out claim must be gone from {owned} too: {got}"
        );
        assert!(
            report
                .files_created_or_updated
                .contains(&(*owned).to_string()),
            "the page the gate rewrote must be reported: {owned} missing from {:?}",
            report.files_created_or_updated
        );
        assert!(
            report.summary.contains(owned),
            "the gate must state the rewrite it performed on Anvil's own {owned}: {}",
            report.summary
        );
    }

    // The other direction, in the same run and on the same bytes.
    for unowned in NOT_OWNED_PAGES {
        assert_eq!(
            std::fs::read_to_string(dir.path().join(unowned)).unwrap(),
            page,
            "{unowned} is not one of Anvil's published corpus pages. It carries the \
             same drifting claims and the same exemption sentence as the five that \
             are, so only the corpus boundary can separate them — and the pipeline \
             commits and pushes whatever this gate edits onto the contributor's \
             branch"
        );
        assert!(
            !report
                .files_created_or_updated
                .contains(&(*unowned).to_string()),
            "{unowned} is not owned, so the gate may not report it as a \
             documentation update — a non-empty file list is read as AutoUpdated \
             and AutoUpdated certifies: {:?}",
            report.files_created_or_updated
        );
    }

    assert!(
        report.is_sufficient,
        "the drift was repaired and the probe judged the diff documented: {}",
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

/// Work done before the probe and work done after it both reach the same report,
/// on both verdicts.
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

/// The skip statement must be observable, and an applied sync must not carry it.
fn an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply() {
    // `!summary.contains(&reason)` alone does not close this, and neither does
    // `console.summary.contains(&anvil.summary)`. An implementation that appends
    // the skip statement UNCONDITIONALLY —
    //
    //     let skip = sync.not_applicable.unwrap_or_default();
    //     let summary = format!("{base}. Corpus sync did not apply: {skip}");
    //
    // satisfies both, because the reason interpolated for Anvil is the empty
    // string:
    //
    //   * anvil.summary   == "BASE. Corpus sync did not apply: "
    //   * console.summary == "BASE. Corpus sync did not apply: REASON"
    //   * console.summary.contains(&reason)        -> true
    //   * !anvil.summary.contains(&reason)         -> true
    //   * console.summary.contains(&anvil.summary) -> TRUE, because the applied
    //     summary is now a literal PREFIX of the skipped one.
    //
    // What ships: gate 1's scorecard row on EVERY Anvil pull request reads
    // "…Corpus sync did not apply:" with nothing after it, while the sync
    // demonstrably applied and rewrote the page. That is the same class of false
    // assurance as issue #27's silent pass, published on the one repository the
    // sync does own.
    //
    // So three further assertions, none of which invents wording:
    //
    //   1. The applied summary must not end mid-statement — no dangling label
    //      character, no trailing whitespace. This kills the whole
    //      `unwrap_or_default()` family whose fixed prose ends in a separator.
    //   2. Whatever text the SKIPPED summary adds must be shared between two
    //      different skipped repositories and must not be the bare reason. Under
    //      the wrong implementation the added text IS the reason, so the two
    //      runs share only whatever prefix the two reasons share; under a correct
    //      one they share the sentence that introduces the reason.
    //   3. That shared sentence, named rather than inferred: the text a skipped
    //      run adds with its own reason removed. It must be the same for both
    //      skipped repositories, it must contain actual words, and it must be
    //      ABSENT from the applied summary. That last clause is the direct,
    //      wording-free statement of the requirement this case is named for, and
    //      it is what kills the one surviving shape of the defect — hoisting the
    //      fixed announcement out of the Option so it lands on every summary and
    //      only the reason stays conditional. See the comment on assertion 3 for
    //      why assertions 1 and 2 both pass that mutant.
    //
    // STATED COST of assertion 3, recorded so it reads as a decision rather than
    // an accident: together with assertion 1 it forbids `format!("{base} {skip}")`
    // — the base summary, a space, and the reason with no announcing words of the
    // gate's own. The argument for forbidding it is that this suite deliberately
    // does NOT pin the wording of `not_applicable` (only that it is non-empty and
    // varies with the repository), so under that shape the entire statement "the
    // sync did not apply" rests on a string nothing requires to say it: a reason
    // of `"not applicable"` satisfies every other assertion here and publishes a
    // scorecard row that reads as an aside. Requiring the gate to contribute the
    // announcement is what makes issue #27's "the skip must not read as a silent
    // pass" enforceable at all. The cost is that one otherwise-blameless
    // composition is out of bounds; the owner may veto this and take the weaker
    // reading, in which case assertion 3's alphabetic clause is the line to drop.
    //
    // STATED REQUIREMENT, so this is a decision and not an artefact: the summary
    // of a *skipped* sync is the summary of an *applied* sync that had nothing
    // to rewrite, plus a statement of the skip APPENDED to it. Pinning that
    // relation, rather than the wording of either, is what makes the skip
    // statement's presence observable without this suite inventing the words it
    // is made of.
    //
    // STATED COST of assertion 2, recorded so it reads as a known trade: it
    // forbids `format!("{base}{skip}")` — the base summary run straight into the
    // reason with no separator at all, not even a space — because that output is
    // byte-identical in shape to the defect it exists to catch. A single space is
    // enough to satisfy it, so the constraint is one character wide.
    //
    // BOTH PROBE VERDICTS, and this is what the case was missing.
    // `ensure_documentation_parity` composes its summary at several independent
    // `format!` sites — one in the `is_doc_sufficient` branch, one after
    // `generate_and_write_docs` — so the skip statement has to be written at
    // least twice, and this relation used to drive only the first of them. The
    // insufficient branch is where the code already reaches for a single
    // unconditional `format!`, which is exactly where the
    // `unwrap_or_default()` family lands:
    //
    //     format!("Documentation is insufficient: {reason}. Files: {files}. \
    //              Corpus sync did not apply: {}", sync.not_applicable.unwrap_or_default())
    //
    // Nothing else in this binary catches that.
    // `the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason`
    // passes on both verdicts because console's reason really is interpolated,
    // and `both_probe_verdicts_carry_anvils_corpus_sync_outcome_into_the_report`
    // asserts only `!summary.contains(&skip_reason)`, which is TRUE when the
    // interpolation is the empty string. What ships is a gate-1 scorecard row on
    // every under-documented Anvil pull request ending `…Corpus sync did not
    // apply:` with nothing after it, while the sync demonstrably applied.
    //
    // SECOND HALF OF THE STATED REQUIREMENT, recorded for the same reason as the
    // first: the base summary — the one an APPLIED sync produces — does not vary
    // with which repository is under review. Only the skip statement does. A
    // summary that named the repository in its base would make the three runs
    // below incomparable, and it would also mean the skip statement could not be
    // told apart from the rest of the sentence by any means this suite has.
    let page = already_honest_page();
    let reason = skipped_sync_reason("oyatie/console");
    let other_reason = skipped_sync_reason("oyatie/oyatie");

    // `[]` is the sufficient verdict: nothing for the probe to ask for, so
    // nothing written, so an empty file list. The stub path is the insufficient
    // verdict, and the stub write is identical in all three checkouts, which is
    // what keeps the three base summaries comparable.
    for stub_files in [&[][..], &["docs/reference/newly-public.md"][..]] {
        let verdict = stub_files.is_empty();
        let outcome = || {
            if stub_files.is_empty() {
                sufficient()
            } else {
                insufficient(Some(MISSING_REASON), stub_files)
            }
        };
        let expected_files: Vec<String> = stub_files.iter().map(|f| (*f).to_string()).collect();

        let anvil_dir = tempdir().unwrap();
        write(&anvil_dir.path().join("README.md"), &page);
        let anvil = run_gate(outcome(), ANVIL, anvil_dir.path(), &["src/lib.rs"]);

        let console_dir = tempdir().unwrap();
        write(&console_dir.path().join("README.md"), &page);
        let console = run_gate(
            outcome(),
            "oyatie/console",
            console_dir.path(),
            &["src/lib.rs"],
        );

        let other_dir = tempdir().unwrap();
        write(&other_dir.path().join("README.md"), &page);
        let other = run_gate(
            outcome(),
            "oyatie/oyatie",
            other_dir.path(),
            &["src/lib.rs"],
        );

        // The fixture is honest already, so no run has anything to REWRITE, and
        // the only files any of them may list are the ones the probe asked for.
        // Asserted for all three so the three base summaries are known to be
        // built out of the same work, which is what the prefix relation below
        // depends on.
        for (label, report) in [
            ("oyatie/anvil", &anvil),
            ("oyatie/console", &console),
            ("oyatie/oyatie", &other),
        ] {
            // Only on the sufficient verdict, and that is deliberate. The
            // insufficient verdict's `is_sufficient: false` is issue #29's
            // headline and is pinned by
            // `an_under_documented_diff_does_not_pass_through_the_public_gate`;
            // asserting it here as well would make this case red for a reason it
            // is not named for, and its red evidence is supposed to read as
            // "the summary lied about the skip".
            if verdict {
                assert!(
                    report.is_sufficient,
                    "{label}: the page publishes TOTAL_GATES and the probe judged \
                     the diff documented: {}",
                    report.summary
                );
            }
            assert!(
                report.errored.is_none(),
                "is_doc_sufficient={verdict} {label}: a judgement was obtained and \
                 the directory is writable, so nothing here is absent evidence: {:?}",
                report.errored
            );
            assert_eq!(
                report.files_created_or_updated, expected_files,
                "is_doc_sufficient={verdict} {label}: the page already publishes \
                 TOTAL_GATES, so the only file any of these runs may report is the \
                 one the probe named. The three summaries are only comparable if \
                 they describe the same work: {}",
                report.summary
            );
            assert!(
                !report.summary.trim().is_empty(),
                "is_doc_sufficient={verdict} {label}: a gate that says nothing \
                 cannot be read"
            );
        }

        assert_eq!(
            std::fs::read_to_string(anvil_dir.path().join("README.md")).unwrap(),
            page,
            "is_doc_sufficient={verdict}: the page already publishes TOTAL_GATES; \
             an applied sync has nothing to do to it"
        );
        assert!(
            console.summary.contains(&reason),
            "is_doc_sufficient={verdict} oyatie/console: the skipped sync must be \
             stated: {}",
            console.summary
        );
        assert!(
            other.summary.contains(&other_reason),
            "is_doc_sufficient={verdict} oyatie/oyatie: the skipped sync must be \
             stated here too: {}",
            other.summary
        );
        assert!(
            !anvil.summary.contains(&reason),
            "is_doc_sufficient={verdict} oyatie/anvil: the sync applied, so the \
             summary must not carry a skipped sync's reason: {}",
            anvil.summary
        );
        assert!(
            console.summary.contains(&anvil.summary),
            "is_doc_sufficient={verdict}: the skipped summary must be the applied \
             summary plus a statement of the skip — nothing about the skip may \
             appear in the applied one.\napplied: {}\nskipped: {}",
            anvil.summary,
            console.summary
        );

        // Assertion 1: the applied summary must not end mid-statement.
        //
        // A summary published to a contributor must not promise a reason it
        // never gives. `format!("{base}. Corpus sync did not apply: {skip}")`
        // with `skip == ""` ends in `": "`; every member of that family ends in
        // the separator its fixed prose put before the interpolation.
        assert_eq!(
            anvil.summary,
            anvil.summary.trim_end(),
            "is_doc_sufficient={verdict} oyatie/anvil: the sync applied, so the \
             summary is complete as it stands and must not trail off into \
             whitespace where an interpolated skip reason would have gone: {:?}",
            anvil.summary
        );
        for dangling in [':', '-', '\u{2013}', '\u{2014}', '(', ',', ';'] {
            assert!(
                !anvil.summary.trim_end().ends_with(dangling),
                "is_doc_sufficient={verdict} oyatie/anvil: the sync applied and \
                 rewrote nothing, so the summary must be a finished statement. \
                 Ending on {dangling:?} means the gate told this Anvil pull \
                 request that something followed — the reason the sync did not \
                 apply — and then said nothing. That is the same false assurance \
                 as issue #27's silent pass, published on the one repository the \
                 sync does own: {:?}",
                anvil.summary
            );
        }

        // Assertion 2: the text the skipped summary adds is the implementation's
        // own introducing phrase, not the bare reason.
        //
        // Two different repositories, so the two reasons differ, so the text they
        // SHARE is exactly the fixed prose the implementation wrote. Under
        // `format!("{prefix}{skip}")` with `unwrap_or_default()` that fixed prose
        // has already been consumed into the applied summary and the remainder is
        // the bare reason, whose only shared prefix is whatever the two reasons
        // happen to share — which is by definition a prefix of both of them.
        let added_for_console = console
            .summary
            .strip_prefix(&anvil.summary)
            .unwrap_or_else(|| {
                panic!(
                    "is_doc_sufficient={verdict}: the skip statement is APPENDED to \
                     the applied summary.\napplied: {}\nskipped: {}",
                    anvil.summary, console.summary
                )
            });
        let added_for_oyatie = other
            .summary
            .strip_prefix(&anvil.summary)
            .unwrap_or_else(|| {
                panic!(
                    "is_doc_sufficient={verdict}: the skip statement is APPENDED to \
                     the applied summary.\napplied: {}\nskipped: {}",
                    anvil.summary, other.summary
                )
            });

        let shared = common_prefix(added_for_console, added_for_oyatie);
        assert!(
            !shared.is_empty(),
            "is_doc_sufficient={verdict}: two skipped repositories must share the \
             phrase that introduces the reason; if they share nothing, the applied \
             summary has already absorbed it.\napplied:  {}\nskipped 1: {}\n\
             skipped 2: {}",
            anvil.summary,
            console.summary,
            other.summary
        );
        assert!(
            !reason.starts_with(shared) && !other_reason.starts_with(shared),
            "is_doc_sufficient={verdict}: the sentence that introduces the skip \
             reason must live on the SKIPPED side, not dangle on the applied one. \
             The two skipped summaries share only {shared:?}, which is the start \
             of the reason itself — so the words that announce it were \
             interpolated into the applied summary too, and this Anvil pull \
             request is being told the sync did not apply while it demonstrably \
             did.\napplied:  {}\nskipped 1: {}\nskipped 2: {}",
            anvil.summary,
            console.summary,
            other.summary
        );

        // Assertion 3: the introducing phrase itself, named and required to be
        // absent from the applied summary.
        //
        // Assertions 1 and 2 leave one shape of the very defect this case is
        // named for alive. HOIST the fixed phrase out of the Option and
        // interpolate only the reason:
        //
        //     let summary = format!(
        //         "{base} Corpus sync did not apply{}",
        //         not_applicable.map(|r| format!(": {r}")).unwrap_or_default(),
        //     );
        //
        // Then `anvil.summary` ends "…Corpus sync did not apply" — a flat false
        // statement published on every Anvil pull request, on the one repository
        // the sync does own — and every assertion above passes it. It ends on
        // `y`, so assertion 1's trim and dangling-character loop pass. It is a
        // literal prefix of both skipped summaries, so `contains` and
        // `strip_prefix` pass. The text each skipped run adds is `": REASON"`,
        // so `shared` is `": oyatie/"`: non-empty, and not a prefix of either
        // reason, so assertion 2 passes too. The whole suite stays green while
        // gate 1's scorecard row tells every Anvil pull request the corpus sync
        // did not apply while it demonstrably did.
        //
        // So name the phrase rather than infer it: the introducer is what the
        // skipped run added with its own reason taken back out. Under a correct
        // implementation that is the fixed prose announcing the skip; under the
        // mutant it is the punctuation `": "` that was left holding the reason
        // after the words were hoisted away.
        //
        // MEASURED, not argued — two runs, both reverted. With a nine-line
        // ownership guard in `sync_published_counts` and
        // `format!("{base} Corpus sync did not apply: {reason}")` on both summary
        // sites, this case passes. Hoisting the fixed phrase out of the Option —
        // `format!("{base} Corpus sync did not apply{}", opt.map(|r| format!(": {r}")).unwrap_or_default())`
        // — leaves assertions 1 and 2 green and dies here, reporting
        // `The skipped run added only ": "` beside an applied summary that ends
        // "...Corpus sync did not apply".
        assert!(
            added_for_console.contains(&reason) && added_for_oyatie.contains(&other_reason),
            "is_doc_sufficient={verdict}: the reason must live in the text the \
             skipped run ADDED, not straddle the boundary with the applied \
             summary.\napplied:  {}\nskipped 1: {}\nskipped 2: {}",
            anvil.summary,
            console.summary,
            other.summary
        );
        let introducer = added_for_console.replace(reason.as_str(), "");
        let other_introducer = added_for_oyatie.replace(other_reason.as_str(), "");
        assert_eq!(
            introducer, other_introducer,
            "is_doc_sufficient={verdict}: with each run's own reason removed, what \
             remains is the implementation's fixed announcement of the skip, and it \
             does not vary with which repository was skipped.\napplied:  {}\n\
             skipped 1: {}\nskipped 2: {}",
            anvil.summary, console.summary, other.summary
        );
        assert!(
            introducer.chars().any(char::is_alphabetic),
            "is_doc_sufficient={verdict}: the gate must say IN WORDS that the sync \
             did not apply. Nothing in this suite pins the wording of \
             `not_applicable`, so a summary that adds only punctuation around it \
             leaves the whole announcement to a string the gate does not own — and \
             the shape that gets here is the one that hoisted the announcement out \
             of the Option and onto the applied summary, where it is false. The \
             skipped run added only {introducer:?}.\napplied:  {}\nskipped 1: {}\n\
             skipped 2: {}",
            anvil.summary,
            console.summary,
            other.summary
        );
        assert!(
            !anvil.summary.contains(&introducer),
            "is_doc_sufficient={verdict}: this is the requirement the case is named \
             for, stated without inventing a word of it — the phrase that announces \
             a skipped sync, {introducer:?}, must be absent from the summary of a \
             sync that APPLIED. Seeing it there means every Anvil pull request is \
             told the corpus sync did not apply while it demonstrably \
             did.\napplied:  {}\nskipped 1: {}\nskipped 2: {}",
            anvil.summary,
            console.summary,
            other.summary
        );
    }
}

/// The longest common prefix of two strings, on character boundaries.
fn common_prefix<'a>(a: &'a str, b: &str) -> &'a str {
    let mut end = 0;
    for ((i, ca), cb) in a.char_indices().zip(b.chars()) {
        if ca != cb {
            return &a[..i];
        }
        end = i + ca.len_utf8();
    }
    &a[..end]
}

/// Anvil's OWN corpus, when it cannot be read, is absent evidence and must
/// error — the counterweight to every "a skipped sync never errors" case above.
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

/// Published drift the sync could not repair fails Anvil's own gate, and the
/// page it rewrote on the way is never reported as work done.
///
/// This is the `Ok(sync) if !sync.remaining_drift.is_empty()` arm of the
/// corpus-sync match. It is the arm this branch is REWRITING — `not_applicable`
/// has to be threaded through that same match — and until now nothing anywhere
/// pinned it: every Anvil fixture in this suite carries drift the rewriter fully
/// repairs, and every non-Anvil fixture asserts empty drift by design.
///
/// The refactor that drops it is not exotic. Threading the skip through is most
/// naturally written as
/// `let sync = sync_published_counts(repo, repo_dir, TOTAL_GATES)?; let skip =
/// sync.not_applicable; let rewritten = sync.rewritten;`, which flattens three
/// arms into one and loses this one as a casualty. Anvil's own gate 1 then stops
/// failing on published drift it could not repair. It does not even go
/// `AutoUpdated`: `rewritten` is non-empty on this fixture, so the page the sync
/// wrote and cannot vouch for is announced as a completed documentation update
/// and the pull request is certified.
///
/// So the probe is given the SUFFICIENT verdict. Everything the gate is told
/// about this diff says pass; the only thing that must stop it is the corpus the
/// sync itself flagged.
fn published_drift_the_sync_could_not_repair_fails_anvils_own_gate() {
    let drift = unrepaired_drift_entry();

    let dir = tempdir().unwrap();
    write(&dir.path().join("README.md"), &unrepairable_drift_page());

    let report = run_gate(sufficient(), ANVIL, dir.path(), &["src/lib.rs"]);

    assert!(
        !report.is_sufficient,
        "the sync wrote Anvil's own README.md and still reports a claim it could \
         not make honest; a page the gate cannot vouch for is not a documented \
         diff, whatever the probe said about the diff itself: {}",
        report.summary
    );
    // Drift is a FINDING, not missing evidence. The sync ran, read the page, and
    // reported what it found, so this must be `Failed`, not `Errored` — the two
    // block alike today, but `Errored` says the gate could not judge, and a gate
    // that cannot distinguish "I found a problem" from "I could not look" cannot
    // be trusted by anything downstream of it.
    assert!(
        report.errored.is_none(),
        "the sync ran and reported a finding; that is an adverse judgement, not \
         absent evidence: {:?}",
        report.errored
    );
    // The heart of it, and the reason the arm returns an empty list rather than
    // `sync.rewritten`: the evaluator maps a non-empty file list onto
    // `GateStatus::AutoUpdated`, and `AutoUpdated.is_acceptable()` is `true`. A
    // report that listed the page it rewrote would certify this pull request
    // while saying, in the same breath, that the page is still wrong.
    assert!(
        report.files_created_or_updated.is_empty(),
        "the sync rewrote README.md but could not finish the job, so nothing may \
         be reported as an update — a non-empty list here is read as AutoUpdated \
         and AutoUpdated certifies: {:?}",
        report.files_created_or_updated
    );
    assert!(
        report.summary.contains(&drift),
        "the gate must name the drift it is failing on so a contributor can fix \
         it; expected the summary to carry {drift:?}, got: {}",
        report.summary
    );
}

// =========================================================================
// Issue #29 — absent or failed evidence is never a pass
// =========================================================================
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

/// The headline: a diff the probe judged under-documented does not come back
/// sufficient, however much work the guard did about it.
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

/// Naming no file to fix does not turn an adverse judgement into a pass.
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

/// Declining to explain does not turn an adverse judgement into a pass either,
/// and the gate may not block with an empty reason.
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
    // unactionable assurance this branch exists to remove.
    // `let summary = eval.missing_doc_summary.clone().unwrap_or_default();`
    // satisfies every other #29 case here and blocks this contributor's PR with
    // `is_sufficient: false, errored: None, summary: ""`.
    assert!(
        !report.summary.trim().is_empty(),
        "a gate that fails a pull request must say something a contributor can \
         act on; the probe declining to explain itself is not a licence for the \
         gate to publish an empty reason: {report:?}"
    );

    // Non-emptiness alone is not enough, and the hole it leaves is the exact
    // defect class its sibling
    // `an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply`
    // spends fifty lines forbidding, one file away. It permits a summary that
    // PROMISES a reason and then gives none:
    //
    //     let summary = format!(
    //         "Documentation is insufficient: {}",
    //         eval.missing_doc_summary.clone().unwrap_or_default()
    //     );
    //
    // With `missing_doc_summary: None` that yields
    // `"Documentation is insufficient: "`. It is non-empty after `trim`, it is
    // byte-different from the passing summary compared against below, and
    // `an_under_documented_diff_does_not_pass_through_the_public_gate` never
    // sees it because the reason IS present there — so the whole suite stays
    // green while every contributor whose probe declines to explain itself gets
    // a blocked scorecard row reading `Documentation is insufficient:` with
    // nothing after the colon. A gate telling a pull request that something
    // follows and then saying nothing is the same false and unactionable
    // assurance as issue #27's silent pass, published on the failing side.
    //
    // So the two assertions the applied-sync summary already carries are applied
    // to this report as well. They invent no wording — they say only that the
    // sentence the implementation chose is FINISHED — and together they kill the
    // whole `unwrap_or_default()` family on this path, exactly as they do on the
    // other.
    assert_eq!(
        report.summary,
        report.summary.trim_end(),
        "the probe gave no reason, so the summary is complete as it stands and \
         must not trail off into whitespace where an interpolated reason would \
         have gone: {:?}",
        report.summary
    );
    for dangling in [':', '-', '\u{2013}', '\u{2014}', '(', ',', ';'] {
        assert!(
            !report.summary.trim_end().ends_with(dangling),
            "the probe declined to explain itself, so the summary must be a \
             finished statement. Ending on {dangling:?} means the gate told this \
             pull request that something followed — the reason the diff is \
             under-documented — and then said nothing: {:?}",
            report.summary
        );
    }

    // And it must at least be DISTINGUISHABLE from the pass it is not. Pinned as
    // a relation rather than as wording, so this suite does not invent the
    // words: the same probe verdict inverted produces the gate's passing
    // summary, and a blocked pull request may not be described byte-for-byte the
    // same way as a passing one.
    //
    // STATED LIMIT, so the assertion is not read as delivering more than it
    // does: byte-inequality is all this pins. Today's
    // `format!("Auto-generated documentation updates for: {files}")` satisfies
    // it while still reading like the AutoUpdated pass state. A wording-free
    // assertion that the summary reads as ADVERSE would need a passing run that
    // also generates files to compare against, and no such run exists — the
    // passing path never has a probe finding to report. The requirement that the
    // summary be actionable is carried by the non-empty assertion above and by
    // `an_under_documented_diff_does_not_pass_through_the_public_gate`, which
    // requires the probe's reason itself to reach the summary.
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

/// A probe that produced no judgement at all is Errored, and never a pass.
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

/// Work the gate genuinely did is not evidence about the diff the probe never
/// judged.
fn a_failed_probe_is_not_rescued_by_a_corpus_sync_that_did_have_work_to_do() {
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
    // every binary goes green.
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

/// A write that never happened is never reported as AutoUpdated.
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
    let errored = report.errored.as_deref().unwrap_or_else(|| {
        panic!(
            "a write that failed is absent evidence and must be Errored. summary \
             was: {}",
            report.summary
        )
    });
    assert!(
        !errored.trim().is_empty(),
        "an Errored gate that states nothing cannot be acted on: {report:?}"
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

/// A failure averaged away with a success is still a failure.
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
    let errored = report.errored.as_deref().unwrap_or_else(|| {
        panic!(
            "one of the writes failed, and a failure that is averaged away with a \
             success is absent evidence: {}",
            report.summary
        )
    });
    assert!(
        !errored.trim().is_empty(),
        "an Errored gate that states nothing cannot be acted on: {report:?}"
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

/// A file that was named but never amended is not reported as updated, and
/// naming it never yields a pass.
fn naming_an_existing_file_that_is_never_amended_cannot_yield_a_pass() {
    let dir = tempdir().unwrap();
    let readme = dir.path().join("README.md");
    // The token `newly_public` is deliberately ABSENT from this fixture. It used
    // to be present ("Nothing here mentions newly_public."), which made the
    // else-branch's `after.contains("newly_public")` unfalsifiable: the
    // unconditional prose-survival assertion below already required that
    // sentence to survive, so the token was in `after` in every reachable
    // outcome whatever the implementation did.
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
// Issue #29 at the source of the judgement — where `Err` is PRODUCED
// =========================================================================
//
// Every case above supplies a probe outcome and pins what the gate does with
// it. None of them runs the code that DECIDES what that outcome is, so until
// now every `Err` this suite consumed was a string a test wrote. The two cases
// here close that: the first drives the real probe path end to end with nothing
// for it to spawn, and the second binds the gate's classification of a probe run
// to the exported function that performs it. `classify_probe_output`'s own
// behaviour — which stdout is a judgement and which is not — is pinned directly
// in `tests/docguard_oracle_repair_test.rs`, where no environment is touched.

/// A probe that could not run at all is Errored — through the real call path,
/// with nothing supplied and nothing spawned.
///
/// FENCE, disclosed: this passes today. It is here because it is the only case
/// in any binary that traverses `evaluate_doc_parity`'s real probe closure, the
/// `run_with_watchdog` call around it, and the watchdog's fallback — the arm
/// whose historical collapse into `is_doc_sufficient: true` made gate 1
/// unfailable. Every `PROBE_FAILURES` case reaches the gate's handling of a
/// failure while leaving the production of one untouched, and the accepted seam
/// design (the override answers at the judgement point, before the watchdog is
/// entered) is what makes that so.
///
/// Nothing is spawned. `PATH` is an empty directory for the whole of this
/// binary, so `Command::new("agy")` fails to resolve and `run_bounded_for`
/// returns before any process exists — which is exactly the "spawn failure"
/// shape `PROBE_FAILURES[0]` writes down by hand, obtained here from the product
/// instead. The failure text is therefore never asserted against a literal: it
/// is read out of the report and required to reach the summary.
fn a_live_probe_that_could_not_run_is_errored_through_the_real_call_path() {
    // Anvil's own repository and an empty checkout, so the corpus sync applies,
    // finds nothing, and returns first with nothing of its own to report. Every
    // subsequent step is live, and an empty `files_created_or_updated` below
    // cannot be a rewritten page in disguise.
    let dir = tempdir().unwrap();
    let ctx = diff_ctx(ANVIL, dir.path(), &["src/lib.rs"]);
    let report = block_on(async {
        DocGuard::new("low".to_string())
            .ensure_documentation_parity(ANVIL, dir.path(), &ctx, "feat: add a public API", "")
            .await
            .expect("a probe that could not run is a report, not a propagated error")
    });

    let errored = report.errored.as_deref().unwrap_or_else(|| {
        panic!(
            "the probe never ran, so nothing was learned about this diff. That is \
             absent evidence and it must be Errored — GateStatus::Errored blocks \
             without claiming the documentation is deficient. summary was: {}",
            report.summary
        )
    });
    assert!(
        !errored.trim().is_empty(),
        "an Errored gate that states nothing cannot be acted on: {errored:?}"
    );
    assert!(
        !report.is_sufficient,
        "a probe that never ran cannot have judged the diff documented: {}",
        report.summary
    );
    assert!(
        report.files_created_or_updated.is_empty(),
        "with no judgement there is no file list to act on: {:?}",
        report.files_created_or_updated
    );
    assert!(
        report.summary.contains(errored),
        "the gate must state why it could not evaluate parity, so the failure can \
         be told apart from a documentation finding. errored: {errored:?}, \
         summary: {}",
        report.summary
    );
}

/// The gate classifies a completed probe run exactly as `classify_probe_output`
/// classifies it.
///
/// This is the binding assertion, and it is the same one
/// `a_stub_written_for_an_under_documented_diff_does_not_certify_through_the_evaluator`
/// makes for `doc_parity_status`: the expectation is not written down here, it
/// is COMPUTED by calling the exported function, and the report the gate
/// produced for the same probe run must agree with it. A second, private copy of
/// the classification inside the probe closure — the shape that reintroduces
/// "ran, printed nothing usable, therefore sufficient" while leaving the
/// exported function correctly repaired, publicly visible and uncalled — cannot
/// diverge without failing here.
///
/// Nothing is spawned: the probe run is supplied, and only the run. What the
/// gate does with it is production code all the way down.
///
/// STATED EXCLUSION: this does not assert that the classification happens
/// *inside* the watchdog-supervised closure, only that it is the exported
/// function's answer that reaches the report. Requiring the watchdog's wrapping
/// would pin how the error is composed rather than what it says, and
/// `a_live_probe_that_could_not_run_is_errored_through_the_real_call_path` above
/// already traverses that closure for real.
///
/// MEASURED, not argued — three runs, all reverted:
///
/// * With the four scaffolding bodies filled and `is_sufficient` corrected on
///   the generate path, this case passes on all four runs. It is satisfiable by
///   an obvious correct implementation.
/// * With a second, private copy in the `SuppliedOutput` arm that collapses
///   "ran, printed nothing usable" into `is_doc_sufficient: true` — the exact
///   historical defect, with the exported function left correctly repaired and
///   uncalled — it fails on "a successful run that printed no judgement" with
///   "Absent evidence is never a pass".
/// * With a private copy that returns `Err` but words it differently, it fails
///   on the same run with the classifier's message and the reported one printed
///   side by side. Divergence in either direction is caught.
fn the_supplied_probe_output_is_classified_by_the_exported_classifier() {
    let runs: &[(&str, i32, &str, &str)] = &[
        ("a judgement of sufficiency", 0, PRINTED_SUFFICIENT, ""),
        ("a judgement of insufficiency", 0, PRINTED_INSUFFICIENT, ""),
        // The historical defect, in the shape the product actually meets it: the
        // probe ran, exited zero, and printed prose. It said nothing about this
        // diff.
        (
            "a successful run that printed no judgement",
            0,
            "I was unable to review this diff.\n",
            "",
        ),
        // A judgement on stdout AND a non-zero exit: the run failed, so its
        // answer is not taken. Nothing else in this suite drives that pairing.
        (
            "a non-zero exit that printed a judgement anyway",
            1,
            PRINTED_SUFFICIENT,
            "permission check failed for command",
        ),
    ];

    for (label, code, stdout, stderr) in runs {
        let expected = classify_probe_output(exit_status(*code), stdout, stderr);

        let dir = tempdir().unwrap();
        let ctx = diff_ctx(ANVIL, dir.path(), &["src/lib.rs"]);
        let report = block_on(async {
            DocGuard::with_probe_output_override(
                "low".to_string(),
                probe_output(*code, stdout, stderr),
            )
            .ensure_documentation_parity(ANVIL, dir.path(), &ctx, "feat: add a public API", "")
            .await
            .expect("the gate reports, it does not propagate")
        });

        match expected {
            Ok(eval) => {
                assert!(
                    report.errored.is_none(),
                    "{label}: `classify_probe_output` obtained a judgement from this \
                     run, so the gate has evidence and this is not absent evidence: \
                     {:?}",
                    report.errored
                );
                assert_eq!(
                    report.is_sufficient, eval.is_doc_sufficient,
                    "{label}: the gate's verdict must be the verdict the exported \
                     classifier read out of this run. summary: {}",
                    report.summary
                );
            }
            Err(e) => {
                let errored = report.errored.as_deref().unwrap_or_else(|| {
                    panic!(
                        "{label}: `classify_probe_output` obtained no judgement from \
                         this run, so the gate has none either. Absent evidence is \
                         never a pass. summary: {}",
                        report.summary
                    )
                });
                assert!(
                    errored.contains(&e.to_string()),
                    "{label}: the reason the gate reports must be the reason the \
                     exported classifier gave, or a second private copy is deciding \
                     this and the exported one is decoration.\nclassifier: {}\n\
                     reported:   {errored}",
                    e
                );
                assert!(
                    !report.is_sufficient,
                    "{label}: no judgement was obtained, so the diff was not judged \
                     documented: {}",
                    report.summary
                );
                assert!(
                    report.files_created_or_updated.is_empty(),
                    "{label}: with no judgement there is no file list to act on: {:?}",
                    report.files_created_or_updated
                );
                assert!(
                    report.summary.contains(errored),
                    "{label}: the summary a contributor reads must carry the reason: {}",
                    report.summary
                );
            }
        }
    }
}

// =========================================================================
// The single entry point
// =========================================================================

/// Every case above, run in one thread with nothing on `PATH` for
/// `Command::new("agy")` to find.
#[test]
fn the_documentation_gate_is_pinned_with_no_agy_reachable_on_path() {
    // SAFETY: this binary contains exactly one `#[test]`, so no other thread of
    // this process is running while the environment is mutated. That is the
    // whole reason the cases below are functions rather than tests of their own.
    //
    // The empty directory is the point: `Command::new("agy")` resolves through
    // `PATH`, so with `PATH` pointing at a directory containing nothing, a
    // fall-through spawn fails immediately with "No such file or directory"
    // rather than starting a model under a 120-second budget with
    // `--dangerously-skip-permissions`. The detection mechanism of every case
    // below is its own assertions; this is what makes the forbidden act
    // unreachable rather than merely unasserted.
    let empty = tempdir().unwrap();
    unsafe {
        std::env::set_var("PATH", empty.path());
    }

    let cases: &[(&str, fn())] = &[
        (
            "reviewing_a_repository_that_is_not_anvil_leaves_its_owned_pages_byte_identical",
            reviewing_a_repository_that_is_not_anvil_leaves_its_owned_pages_byte_identical,
        ),
        (
            "a_finding_the_gate_reached_before_the_probe_is_the_finding_it_reports",
            a_finding_the_gate_reached_before_the_probe_is_the_finding_it_reports,
        ),
        (
            "the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason",
            the_gate_summary_for_a_non_anvil_repository_carries_the_skipped_syncs_reason,
        ),
        (
            "the_gate_applies_the_corpus_sync_to_anvils_own_repository",
            the_gate_applies_the_corpus_sync_to_anvils_own_repository,
        ),
        (
            "both_probe_verdicts_carry_anvils_corpus_sync_outcome_into_the_report",
            both_probe_verdicts_carry_anvils_corpus_sync_outcome_into_the_report,
        ),
        (
            "an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply",
            an_applied_corpus_sync_is_never_described_as_one_that_did_not_apply,
        ),
        (
            "a_corpus_sync_that_could_not_run_at_all_is_errored_at_the_gate",
            a_corpus_sync_that_could_not_run_at_all_is_errored_at_the_gate,
        ),
        (
            "published_drift_the_sync_could_not_repair_fails_anvils_own_gate",
            published_drift_the_sync_could_not_repair_fails_anvils_own_gate,
        ),
        (
            "an_under_documented_diff_does_not_pass_through_the_public_gate",
            an_under_documented_diff_does_not_pass_through_the_public_gate,
        ),
        (
            "an_under_documented_diff_that_named_no_files_still_fails_the_gate",
            an_under_documented_diff_that_named_no_files_still_fails_the_gate,
        ),
        (
            "an_under_documented_diff_that_stated_no_reason_still_fails_the_gate",
            an_under_documented_diff_that_stated_no_reason_still_fails_the_gate,
        ),
        (
            "a_probe_that_produced_no_judgement_is_errored_and_never_a_pass",
            a_probe_that_produced_no_judgement_is_errored_and_never_a_pass,
        ),
        (
            "a_failed_probe_is_not_rescued_by_a_corpus_sync_that_did_have_work_to_do",
            a_failed_probe_is_not_rescued_by_a_corpus_sync_that_did_have_work_to_do,
        ),
        (
            "a_documentation_write_that_failed_is_errored_and_never_reported_as_updated",
            a_documentation_write_that_failed_is_errored_and_never_reported_as_updated,
        ),
        (
            "a_write_that_failed_is_not_reported_as_updated_even_when_another_one_succeeded",
            a_write_that_failed_is_not_reported_as_updated_even_when_another_one_succeeded,
        ),
        (
            "naming_an_existing_file_that_is_never_amended_cannot_yield_a_pass",
            naming_an_existing_file_that_is_never_amended_cannot_yield_a_pass,
        ),
        (
            "a_live_probe_that_could_not_run_is_errored_through_the_real_call_path",
            a_live_probe_that_could_not_run_is_errored_through_the_real_call_path,
        ),
        (
            "the_supplied_probe_output_is_classified_by_the_exported_classifier",
            the_supplied_probe_output_is_classified_by_the_exported_classifier,
        ),
    ];

    // `catch_unwind` so that one red case does not hide the other seventeen: a
    // run of this binary reports every behaviour that is failing, which is the
    // property a single aggregating test would otherwise cost.
    let mut failures: Vec<String> = Vec::new();
    for (name, case) in cases {
        match catch_unwind(AssertUnwindSafe(case)) {
            Ok(()) => eprintln!("case {name} ... ok"),
            Err(payload) => {
                let message = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
                    .unwrap_or_else(|| "<non-string panic payload>".to_string());
                eprintln!("case {name} ... FAILED");
                failures.push(format!("  {name}\n    {message}"));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} gate cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}
