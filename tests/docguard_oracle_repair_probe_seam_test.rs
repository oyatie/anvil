//! The probe seam's own contract: the override is a stored value, and it may
//! never fall through to a real `agy` spawn.
//!
//! `tests/docguard_oracle_repair_gate_test.rs` drives every other gate case, and
//! every one of them constructs a fresh guard and calls the gate exactly once —
//! which is precisely what makes a one-shot slot indistinguishable from a stored
//! value. That binary empties `PATH` so a fall-through spawn cannot reach a
//! model; it does not, and cannot, say anything about the override surviving a
//! second call.
//!
//! ## Why this is a separate binary
//!
//! The behaviour is pinned by running the gate TWICE on one guard. Against the
//! wrong implementation it exists to catch — `probe_override:
//! Mutex<Option<Result<..>>>` with a `take()` at the top of
//! `evaluate_doc_parity`, falling through to the real spawn once drained — the
//! second call is the one that would run
//! `agy --print <prompt> --effort low --dangerously-skip-permissions` under a
//! 120-second `run_bounded_for` budget, from inside `cargo test`.
//!
//! A test whose detection mechanism is the act it forbids is the same shape the
//! gate suite criticises in
//! `reviewing_a_repository_that_is_not_anvil_leaves_its_owned_pages_byte_identical`:
//! a breach detected only after the model has already run. It also makes the
//! outcome depend on things that have nothing to do with the behaviour — whether
//! `agy` is on `PATH`, whether the network is up, what the model returns, how
//! long it takes. On a clean CI box it fails fast for the right reason; on a
//! developer machine with `agy` installed it burns up to 120 seconds per
//! outcome, three outcomes in the loop, and only then reports.
//!
//! So `PATH` is emptied before the gate runs. A fall-through spawn then fails in
//! microseconds with "No such file or directory" instead of invoking a model,
//! and the assertions below still see exactly what they were written to see.
//! Mutating the environment is a data race in a parallel test binary, which is
//! why this case is alone in its own — the same reason
//! `tests/docguard_oracle_repair_self_repo_test.rs` is, and the same reason
//! `tests/docguard_oracle_repair_gate_test.rs` runs its eighteen cases from a
//! single `#[test]`.
//!
//! ## Agreement is not the assertion; the stored value is
//!
//! An earlier version of this case asserted only that the two runs agreed —
//! `first.is_sufficient == second.is_sufficient`, and the same for `errored` and
//! `summary`. That was vacuous against the one implementation this file exists
//! to catch. An override that is stored and never consulted
//! (`Probe::Overridden(_) => "low".to_string()`, falling through to the spawn on
//! every call) makes both runs identical: both spawn, both fail the same way,
//! both come back `is_sufficient: false` with the same `errored` and the same
//! `summary`. All three relational assertions passed it.
//!
//! So each outcome now carries an ABSOLUTE check, applied to both runs, that
//! only the stored value can satisfy: the supplied verdict on the two `Ok` arms,
//! and on the `Err` arm the `seam-sentinel:` string in `PROBE_FAILURE`, which is
//! deliberately unproducible by any real spawn. The agreement assertions stay,
//! because they are what catches the *other* mistake — an override that answers
//! once and then stops.
//!
//! ## The second closure, which needs no test
//!
//! `DocGuard`'s stored outcome is declared as `doc_guard::Probe`:
//!
//! ```ignore
//! pub enum Probe {
//!     Live(String),
//!     Overridden(Result<DocParityEvaluation, String>),
//! }
//! ```
//!
//! It has no empty arm and no drainable arm, so an implementer cannot reach for
//! `take()` — there is no `None` to fall through from, and "the override has
//! been used up" is not a state this code can spell. The contract doc on
//! `with_probe_override` says so as a requirement; `Probe` is what stops the
//! mistake from compiling. This case remains because the type constrains where
//! the outcome is *stored*, not that `evaluate_doc_parity` keeps consulting it.

use anvil::doc_guard::{DocGuard, DocGuardReport, DocParityEvaluation};
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::report::TOTAL_GATES;
use std::path::Path;
use tempfile::tempdir;

const ANVIL: &str = "oyatie/anvil";

/// Every page `collect_owned_pages` claims. The fixture below is written to all
/// five rather than to `README.md` alone, so the fence that neither run changes
/// the checkout covers every owned page kind — including the two that are not
/// markdown or not at the repository root.
const OWNED_PAGES: &[&str] = &[
    "README.md",
    "docs/doctrine.md",
    "openapi/openapi.yaml",
    "docs/adr/0001-console.md",
    "docs/decisions/0001-console.md",
];

/// Mirrors `MISSING_REASON` in the gate suite.
const MISSING_REASON: &str = "newly_public is a new public API with no reference page";

/// A probe failure that **no real spawn can produce**, deliberately.
///
/// It used to mirror `PROBE_FAILURES[0]` in the gate suite
/// ("failed to run doc parity probe: No such file or directory (os error 2)"),
/// which was a latent trap: `run_bounded_for` really does emit
/// "doc parity probe failed to run: No such file or directory (os error 2)" —
/// the same words in a different order — so a live ENOENT from the emptied
/// `PATH` was one wording change away from being indistinguishable from the
/// stored value. The `seam-sentinel:` prefix removes that possibility: nothing
/// but `with_probe_override` can put this string into a report, so an assertion
/// that the report carries it can only be satisfied by the STORED outcome having
/// been consulted.
const PROBE_FAILURE: &str = "seam-sentinel: the doc parity probe outcome was supplied by the test";

/// Mirrors `already_honest_page()` in the gate suite: a page with nothing at all
/// for the corpus sync to do, so neither run can change the checkout and any
/// difference between the two reports is the seam's.
fn already_honest_page() -> String {
    format!("# Page\n\nShips behind a {TOTAL_GATES}-gate release check.\n")
}

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, body).unwrap();
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
        repo_working_dir: anvil::git_manager::SubjectRoot::asserted(
            repo_dir.to_path_buf(),
            anvil::git_manager::Uncloned::TestFixture,
        ),
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

fn probe_failed(reason: &str) -> Result<DocParityEvaluation, String> {
    Err(reason.to_string())
}

/// Drives the public gate **twice on one guard**, so an override that empties
/// after the first read is observable.
///
/// Every helper in the gate suite constructs a fresh guard per call, which is
/// exactly what makes a one-shot override indistinguishable from a stored one.
fn run_gate_twice(
    outcome: Result<DocParityEvaluation, String>,
    repo: &str,
    repo_dir: &Path,
    changed: &[&str],
) -> (DocGuardReport, DocGuardReport) {
    let ctx = diff_ctx(repo, repo_dir, changed);
    block_on(async {
        let guard = DocGuard::with_probe_override(outcome);
        let first = guard
            .ensure_documentation_parity(repo, repo_dir, &ctx, "feat: add a public API", "")
            .await
            .unwrap();
        let second = guard
            .ensure_documentation_parity(repo, repo_dir, &ctx, "feat: add a public API", "")
            .await
            .unwrap();
        (first, second)
    })
}

#[test]
fn the_probe_outcome_is_not_consumed_by_the_first_run_of_the_gate() {
    // SAFETY: this binary contains exactly one test, so no other thread of this
    // process is running while the environment is mutated. That is also the
    // reason the case lives here instead of in the gate suite.
    //
    // The empty directory is the point: `Command::new("agy")` resolves through
    // `PATH`, so with `PATH` pointing at a directory containing nothing, a
    // fall-through spawn fails immediately with "No such file or directory"
    // rather than starting a model under a 120-second budget. This test's
    // detection mechanism is the assertions below, not the spawn.
    let empty = tempdir().unwrap();
    unsafe {
        std::env::set_var("PATH", empty.path());
    }

    // Pinned behaviourally rather than structurally, in two layers, because
    // agreement alone is not enough.
    //
    // The ABSOLUTE layer (`check`, applied to both runs) says what each run must
    // have come back with. It is the layer that catches the single
    // implementation this binary's header says it exists to catch: an override
    // that is stored and never consulted —
    // `Probe::Overridden(_) => "low".to_string()`, falling straight through to
    // the real spawn on every call. That mutation makes both runs identical
    // (both spawn, both fail the same way), so a suite of `first == second`
    // assertions passes it unchanged. Every absolute assertion below is written
    // so that only the STORED outcome can satisfy it: the two `Ok` arms require
    // the supplied verdict, and the `Err` arm requires the `seam-sentinel:`
    // string, which no spawn can emit.
    //
    // The RELATIONAL layer says the two runs agree, which is what catches an
    // override that answers once and then stops answering.
    //
    // The fixture is `already_honest_page()` on Anvil's own README: the sync
    // applies and has nothing to rewrite, so the second run starts from a
    // byte-identical checkout and the only thing that can differ between the two
    // reports is the seam. All three probe outcomes are exercised, because a slot
    // that empties does so on the `Ok` and `Err` arms alike, and none of these
    // three names a file, so neither run writes anything either.
    let page = already_honest_page();

    type Check = Box<dyn Fn(&DocGuardReport, &str)>;
    let cases: Vec<(&str, Result<DocParityEvaluation, String>, Check)> = vec![
        (
            "sufficient",
            sufficient(),
            Box::new(|report: &DocGuardReport, run: &str| {
                assert!(
                    report.is_sufficient,
                    "sufficient/{run}: the supplied outcome judged this diff \
                     documented, the sync has nothing to do to this page and the \
                     changed file carries no frontmatter, so the report must come \
                     back sufficient. An implementation that stores the override and \
                     never reads it falls through to the spawn `PATH` has been \
                     emptied of, reports is_sufficient: false, and still agrees with \
                     itself on both runs. got: {report:?}"
                );
                assert!(
                    report.errored.is_none(),
                    "sufficient/{run}: a judgement was supplied and nothing failed, so \
                     nothing is absent evidence. got: {report:?}"
                );
            }),
        ),
        (
            "insufficient",
            insufficient(Some(MISSING_REASON), &[] as &[&str]),
            Box::new(|report: &DocGuardReport, run: &str| {
                assert!(
                    !report.is_sufficient,
                    "insufficient/{run}: the supplied outcome judged this diff \
                     under-documented. got: {report:?}"
                );
                assert!(
                    report.summary.contains(MISSING_REASON),
                    "insufficient/{run}: the supplied finding must reach the report, \
                     which is the only way to tell the stored outcome apart from a \
                     verdict the gate produced some other way. got: {report:?}"
                );
                assert!(
                    report.errored.is_none(),
                    "insufficient/{run}: a judgement was supplied, so this is an \
                     adverse finding and not absent evidence. got: {report:?}"
                );
            }),
        ),
        (
            "probe failed",
            probe_failed(PROBE_FAILURE),
            Box::new(|report: &DocGuardReport, run: &str| {
                let errored = report.errored.as_deref().unwrap_or_else(|| {
                    panic!(
                        "probe failed/{run}: the supplied outcome was a probe that \
                         produced no judgement, which is Errored. got: {report:?}"
                    )
                });
                assert!(
                    errored.contains(PROBE_FAILURE),
                    "probe failed/{run}: {PROBE_FAILURE:?} is a sentinel no spawn can \
                     emit, so requiring it here is the assertion that can only be \
                     satisfied by the STORED outcome. A report carrying anything else \
                     — an ENOENT from the emptied `PATH`, for instance — is a run that \
                     never consulted the override. got: {report:?}"
                );
                assert!(
                    !report.is_sufficient,
                    "probe failed/{run}: no judgement was produced, so the diff cannot \
                     have been judged documented. got: {report:?}"
                );
            }),
        ),
    ];

    for (label, outcome, check) in cases {
        let dir = tempdir().unwrap();
        for owned in OWNED_PAGES {
            write(&dir.path().join(owned), &page);
        }

        let (first, second) = run_gate_twice(outcome, ANVIL, dir.path(), &["src/lib.rs"]);

        check(&first, "first run");
        check(&second, "second run");

        for owned in OWNED_PAGES {
            assert_eq!(
                std::fs::read_to_string(dir.path().join(owned)).unwrap(),
                page,
                "{label}: fence — {owned} already publishes TOTAL_GATES, so neither \
                 run may change it and any difference between the two reports below \
                 is the seam's, not the checkout's"
            );
        }
        for report in [&first, &second] {
            assert!(
                report.files_created_or_updated.is_empty(),
                "{label}: every owned page already publishes TOTAL_GATES and no \
                 outcome here names a file, so nothing may be reported as \
                 touched: {:?}",
                report.files_created_or_updated
            );
        }
        assert_eq!(
            first.is_sufficient, second.is_sufficient,
            "{label}: the same guard was asked the same question twice and gave \
             two different verdicts, so the stored probe outcome did not survive \
             the first run. There is no state in which falling through to a real \
             `agy` spawn is legal.\nfirst: {first:?}\nsecond: {second:?}"
        );
        assert_eq!(
            first.errored, second.errored,
            "{label}: the evidence status changed between two runs of one guard, \
             so the override was consumed.\nfirst: {first:?}\nsecond: {second:?}"
        );
        assert_eq!(
            first.summary, second.summary,
            "{label}: the same guard accounted for the same run two different \
             ways, so the override was consumed.\nfirst: {first:?}\nsecond: \
             {second:?}"
        );
    }
}
