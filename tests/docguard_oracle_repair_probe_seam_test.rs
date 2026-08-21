//! The probe seam's own contract: the override is a stored value, and it may
//! never fall through to a real `agy` spawn.
//!
//! `tests/docguard_oracle_repair_test.rs` claims, in its own header, that
//! routing every gate case through `DocGuard::with_probe_override` makes an
//! `agy` spawn "structurally unreachable rather than merely unlikely". Nothing
//! in that binary enforces it, because every case there constructs a fresh guard
//! and calls the gate exactly once — which is precisely what makes a one-shot
//! slot indistinguishable from a stored value.
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
//! main suite criticises in
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
//! and the assertions below still see exactly what they were written to see: two
//! runs of one guard that do not agree. Mutating the environment is a data race
//! in a parallel test binary, which is why this case is alone in its own — the
//! same reason `tests/docguard_oracle_repair_self_repo_test.rs` is.
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

/// Mirrors `MISSING_REASON` in the main suite.
const MISSING_REASON: &str = "newly_public is a new public API with no reference page";

/// Mirrors `PROBE_FAILURES[0]` in the main suite: a probe that never ran.
const PROBE_FAILURE: &str =
    "failed to run doc parity probe: No such file or directory (os error 2)";

/// Mirrors `already_honest_page()` in the main suite: a page with nothing at all
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

fn probe_failed(reason: &str) -> Result<DocParityEvaluation, String> {
    Err(reason.to_string())
}

/// Drives the public gate **twice on one guard**, so an override that empties
/// after the first read is observable.
///
/// Every helper in the main suite constructs a fresh guard per call, which is
/// exactly what makes a one-shot override indistinguishable from a stored one.
fn run_gate_twice(
    outcome: Result<DocParityEvaluation, String>,
    repo: &str,
    repo_dir: &Path,
    changed: &[&str],
) -> (DocGuardReport, DocGuardReport) {
    let ctx = diff_ctx(repo, repo_dir, changed);
    block_on(async {
        let guard = DocGuard::with_probe_override("low".to_string(), outcome);
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
    // reason the case lives here instead of in the main suite.
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

    // Pinned behaviourally rather than structurally: two runs of the same guard
    // must produce the same judgement, the same evidence status, and the same
    // account of both. Nothing here inspects how the override is stored.
    //
    // The fixture is `already_honest_page()` on Anvil's own README: the sync
    // applies and has nothing to rewrite, so the second run starts from a
    // byte-identical checkout and the only thing that can differ between the two
    // reports is the seam. All three probe outcomes are exercised, because a slot
    // that empties does so on the `Ok` and `Err` arms alike, and none of these
    // three names a file, so neither run writes anything either.
    let page = already_honest_page();

    for (label, outcome) in [
        ("sufficient", sufficient()),
        (
            "insufficient",
            insufficient(Some(MISSING_REASON), &[] as &[&str]),
        ),
        ("probe failed", probe_failed(PROBE_FAILURE)),
    ] {
        let dir = tempdir().unwrap();
        write(&dir.path().join("README.md"), &page);

        let (first, second) = run_gate_twice(outcome, ANVIL, dir.path(), &["src/lib.rs"]);

        assert_eq!(
            std::fs::read_to_string(dir.path().join("README.md")).unwrap(),
            page,
            "{label}: fence — this page already publishes TOTAL_GATES, so neither \
             run may change it and any difference between the two reports below \
             is the seam's, not the checkout's"
        );
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
