//! Lane `tdd/trace-gate`: gate 17 (W3C TraceContext) publishes a verification it
//! never performed, and misses the async boundaries it claims to look for.
//!
//! # The defect, restated from source
//!
//! `TraceContextGuard::evaluate_trace_propagation`
//! (`src/trace_context_guard/mod.rs`) splits the diff on `diff --git`, skips any
//! chunk whose text does not contain `.rs`, counts every line containing the
//! literal `tokio::spawn`, asks `SpanTracker` whether each is followed within
//! five lines by `.instrument(`, and then derives its whole verdict from
//! emptiness:
//!
//! ```ignore
//! let is_propagated = detached_findings.is_empty();
//! let summary = if is_propagated {
//!     format!(
//!         "PASSED (W3C trace context & span instrumentation verified across {} async boundaries)",
//!         tasks_scanned
//!     )
//! ```
//!
//! Three separate wrongs fall out of that, and this file pins each one.
//!
//! ## 1. A verification claim with no measurement behind it
//!
//! When nothing was inspected the finding set is empty, so the gate publishes
//! the word *verified* over a count of zero. Issue #14 files this as the
//! non-Rust case. It is wider than that: the chunk filter is the *first* of two
//! ways to inspect nothing, and the second is far more common. A pull request
//! that is entirely Rust and spawns no task -- which is nearly every pull
//! request in this repository -- reaches the same sentence by the other route,
//! with `tasks_scanned == 0` and the same claim attached to it. The tests here
//! cover both routes, and they assert on the published *sentence*, separately
//! from any status enum, because the sentence is what a reviewer reads.
//!
//! ## 2. The status for the nothing-in-scope case is deliberately NOT pinned
//!
//! This repository contains two settled precedents that disagree, and this lane
//! is not entitled to pick between them by writing a test:
//!
//!   - `src/slo_canary_guard/mod.rs:147-159` -- no telemetry source, so
//!     `GateStatus::NotMeasured { gate_id: "slo_status", reason }`.
//!   - `src/coverage_guard.rs:139` -- `CoverageMeasurement::NothingToMeasure`
//!     maps to `GateStatus::Passed`, on the stated ground that a diff adding no
//!     coverable line has nothing to cover.
//!
//! They differ because the situations differ: SLO burn rate is a measurement
//! that *should* have been available and was not, while coverage of zero added
//! lines is a measurement that is complete and empty. A docs-only pull request
//! is the second kind; the choice still has a real cost either way, and it is
//! recorded as an open question for the owner rather than smuggled in here.
//!
//! What both precedents agree on is pinned, because agreement is not a choice:
//!
//!   - neither publishes a sentence claiming a verification that did not happen;
//!   - neither accuses a pull request of a defect it did not commit;
//!   - a real, measured defect still fails.
//!
//! So the nothing-in-scope tests below assert over the summary and the findings
//! and say nothing about which of `Passed` / `NotMeasured` is correct. They pass
//! under either resolution.
//!
//! ## 3. The scanner misses almost every spawn form, and miscounts the rest
//!
//! `SpanTracker::scan_detached_tasks` (`span_tracker.rs:28`) matches the single
//! regex `tokio::spawn\s*\(`. It therefore does not see `tokio::task::spawn`,
//! `tokio::task::spawn_blocking`, `JoinSet::spawn`, or `std::thread::spawn`.
//! That last one is not hypothetical: `src/predictive_test_selector/workspace_dag.rs`
//! spawns two uninstrumented reader threads (verified on this checkout at lines
//! 38 and 45), and the gate reports a file containing them as clean. The fixture
//! for that case is lifted out of the live file at test time rather than copied,
//! so it cannot quietly go stale.
//!
//! In the other direction the counter is inflated: `tasks_scanned` increments
//! for every line containing the literal, `-` removal lines included, so a pull
//! request that *deletes* spawns reports having inspected them -- and, worse,
//! `scan_detached_tasks` reads the same removal lines and accuses the author of
//! introducing the very thing they removed.
//!
//! ## Why the evaluator is scanned as text
//!
//! `evaluator.rs:292-296` rebuilds the verdict as `if trace_report.is_propagated
//! { Passed } else { Failed }`. A boolean carries two outcomes, so any third
//! outcome the guard learns to report is destroyed on the way to the scorecard.
//! That is wiring, invisible to a guard-level test, and the repository already
//! has the idiom for it in `tests/evaluator_preserves_gate_verdicts_test.rs`.
//! Calling `evaluate_pre_merge_gates` for real would mean constructing all
//! seventy-odd sibling reports, which no test in this repository does.
//!
//! ## Out of scope
//!
//! The `.rs`-substring chunk filter is crude in both directions (a Markdown file
//! that mentions `.rs` is scanned; a Rust file renamed in a chunk that does not)
//! but was not in this lane's verified scope, and nothing here asserts about it.
//! No gate is added, no report field is added, `TOTAL_GATES` is untouched.

use anvil::git_manager::PrDiffContext;
use anvil::trace_context_guard::{TraceContextGuard, TraceContextReport};
use std::path::{Path, PathBuf};

// -------------------------------------------------------------------------
// Fixtures
// -------------------------------------------------------------------------

/// Builds a diff in the shape the guard actually parses: one `diff --git`
/// header per file, followed by the `---`/`+++`/`@@` headers a real patch
/// carries, followed by body lines that already carry their own `+`, `-` or
/// leading-space prefix.
fn diff_of(files: &[(&str, &str)]) -> PrDiffContext {
    let mut diff_content = String::new();
    for (path, body) in files {
        diff_content.push_str(&format!(
            "diff --git a/{p} b/{p}\nindex 1111111..2222222 100644\n--- a/{p}\n+++ b/{p}\n@@ -1,8 +1,8 @@\n{body}\n",
            p = path
        ));
    }
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 14,
        base_branch: "main".to_string(),
        base_sha: "aaaaaaa".to_string(),
        head_sha: "bbbbbbb".to_string(),
        diff_content,
        changed_files: files.iter().map(|(p, _)| p.to_string()).collect(),
        repo_working_dir: PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

fn run(diff_ctx: &PrDiffContext) -> TraceContextReport {
    TraceContextGuard::new()
        .evaluate_trace_propagation(Path::new("."), diff_ctx)
        .expect("the trace gate must run to completion on a well-formed diff")
}

/// Prefixes every line of a code block as an addition, so it reads as a hunk
/// that this pull request introduced.
fn as_added(code: &str) -> String {
    code.lines()
        .map(|l| format!("+{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

// -------------------------------------------------------------------------
// Reading the published sentence
// -------------------------------------------------------------------------

/// Words by which a summary asserts that the gate looked and found the code
/// sound. Not a list of forbidden spellings for its own sake: each of these
/// makes a claim about evidence, and a gate that inspected nothing holds none.
/// `verified` is the word in the shipping string; the rest are the synonyms a
/// rewrite would reach for first, so that fixing the defect means removing the
/// claim rather than renaming it.
const VERIFICATION_CLAIMS: &[&str] = &[
    "verified",
    "verifies",
    "verification",
    "confirmed",
    "validated",
    "ensured",
    "instrumentation is sound",
];

fn verification_claims_in(summary: &str) -> Vec<&'static str> {
    let lowered = summary.to_lowercase();
    VERIFICATION_CLAIMS
        .iter()
        .copied()
        .filter(|needle| lowered.contains(needle))
        .collect()
}

/// Ways a summary can state, in plain words, that nothing was looked at. Any
/// one of them is enough; the point is that the reader is told, not that a
/// particular phrasing is used.
///
/// A bare numeral is deliberately not on this list. The shipping string reads
/// "verified across 0 async boundaries" -- it contains the count and still
/// tells the reader the opposite of the truth, because the count is embedded in
/// a claim rather than stated as a finding. Accepting `0` as a disclosure would
/// have let this file's own assertion pass on the defect it exists to catch.
const DISCLOSURES_OF_NOTHING: &[&str] = &[
    "not measured",
    "nothing",
    "no async",
    "no task",
    "no spawn",
    "none",
    "zero",
    "no rust",
];

fn discloses_that_nothing_was_inspected(summary: &str) -> bool {
    let lowered = summary.to_lowercase();
    DISCLOSURES_OF_NOTHING
        .iter()
        .any(|needle| lowered.contains(needle))
}

/// Words by which a summary accuses the pull request of a defect.
const ACCUSATIONS: &[&str] = &["failed", "detached", "drop distributed", "violation"];

fn accusations_in(summary: &str) -> Vec<&'static str> {
    let lowered = summary.to_lowercase();
    ACCUSATIONS
        .iter()
        .copied()
        .filter(|needle| lowered.contains(needle))
        .collect()
}

// -------------------------------------------------------------------------
// 1. No verification claim without a measurement
// -------------------------------------------------------------------------

#[test]
fn a_pull_request_with_no_rust_in_it_claims_no_instrumentation_was_verified() {
    let report = run(&diff_of(&[(
        "docs/adr/0002-honesty.md",
        "+The published name must match the live measurement.",
    )]));

    assert_eq!(
        report.tasks_scanned, 0,
        "no async boundary exists in a Markdown-only diff"
    );
    assert!(
        verification_claims_in(&report.summary).is_empty(),
        "nothing was inspected, so the gate holds no evidence to claim; \
         it published {:?} in: {}",
        verification_claims_in(&report.summary),
        report.summary
    );
}

#[test]
fn a_rust_pull_request_that_spawns_nothing_claims_no_instrumentation_was_verified() {
    // The common case, and the one issue #14 understates: the diff IS Rust, so
    // the chunk filter lets it through, and the gate still measures nothing
    // because the file spawns no task.
    let report = run(&diff_of(&[(
        "src/compute.rs",
        "+pub fn compute(rows: &[u32]) -> u32 {\n+    rows.iter().sum()\n+}",
    )]));

    assert_eq!(
        report.tasks_scanned, 0,
        "this diff crosses no async boundary"
    );
    assert!(
        verification_claims_in(&report.summary).is_empty(),
        "a Rust diff that spawns nothing was still not inspected for span \
         instrumentation; the gate published {:?} in: {}",
        verification_claims_in(&report.summary),
        report.summary
    );
}

#[test]
fn a_gate_that_inspected_nothing_says_so_plainly_and_accuses_no_one() {
    // Both halves of honest reporting, over both routes into an empty
    // measurement. This test is deliberately silent on which GateStatus the
    // nothing-in-scope case deserves: it passes whether the owner chooses
    // NotMeasured (the slo_canary_guard precedent) or Passed (the
    // coverage_guard precedent), because both precedents publish a sentence
    // that satisfies exactly these two conditions.
    for (label, files) in [
        (
            "a documentation-only pull request",
            &[("README.md", "+Anvil is a pre-merge quality matrix.")][..],
        ),
        (
            "a Rust pull request that spawns nothing",
            &[("src/report.rs", "+pub const TOTAL: usize = 72;")][..],
        ),
    ] {
        let report = run(&diff_of(files));

        assert!(
            discloses_that_nothing_was_inspected(&report.summary),
            "{label}: the reader must be told that no async boundary was \
             inspected, not left to infer it; summary was: {}",
            report.summary
        );
        assert!(
            accusations_in(&report.summary).is_empty(),
            "{label}: an empty measurement is not a defect in the pull request; \
             the gate published the accusation {:?} in: {}",
            accusations_in(&report.summary),
            report.summary
        );
        assert!(
            report.detached_findings.is_empty(),
            "{label}: the gate reported {} finding(s) against a diff containing \
             no spawn at all",
            report.detached_findings.len()
        );
    }
}

// -------------------------------------------------------------------------
// 2. Every form of task spawn is an async boundary
// -------------------------------------------------------------------------

#[test]
fn every_form_of_task_spawn_in_use_here_is_inspected_and_an_instrumented_one_is_clean() {
    // Rows are (label, code, expects_a_finding). The negative rows are what
    // stops this being satisfied by flagging the word `spawn` unconditionally:
    // a spawn that carries its span is not a defect, and must not be reported
    // as one.
    let cases: &[(&str, &str, bool)] = &[
        (
            "tokio::spawn",
            "pub async fn dispatch() {\n    tokio::spawn(async move {\n        work().await;\n    });\n}",
            true,
        ),
        (
            "tokio::task::spawn",
            "pub async fn dispatch() {\n    tokio::task::spawn(async move {\n        work().await;\n    });\n}",
            true,
        ),
        (
            "tokio::task::spawn_blocking",
            "pub async fn dispatch() {\n    tokio::task::spawn_blocking(move || {\n        heavy_work();\n    });\n}",
            true,
        ),
        (
            "JoinSet::spawn",
            "pub async fn dispatch() {\n    let mut set = tokio::task::JoinSet::new();\n    set.spawn(async move {\n        work().await;\n    });\n}",
            true,
        ),
        (
            "std::thread::spawn",
            "pub fn dispatch() {\n    let handle = std::thread::spawn(move || {\n        drain_pipe();\n    });\n    let _ = handle.join();\n}",
            true,
        ),
        (
            "tokio::spawn carrying a span",
            "pub async fn dispatch() {\n    tokio::spawn(async move {\n        work().await;\n    }.instrument(tracing::info_span!(\"worker\")));\n}",
            false,
        ),
        (
            "tokio::task::spawn carrying a span",
            "pub async fn dispatch() {\n    tokio::task::spawn(async move {\n        work().await;\n    }.instrument(tracing::info_span!(\"worker\")));\n}",
            false,
        ),
    ];

    let mut unseen = Vec::new();
    let mut misaccused = Vec::new();

    for (label, code, expects_a_finding) in cases {
        let report = run(&diff_of(&[("src/dispatch.rs", &as_added(code))]));
        let found = !report.detached_findings.is_empty();

        if *expects_a_finding && !found {
            unseen.push(*label);
        }
        if !*expects_a_finding && found {
            misaccused.push(*label);
        }
        if *expects_a_finding && found {
            assert!(
                report.tasks_scanned >= 1,
                "{label}: a boundary reported as detached must also have been \
                 counted as inspected, or `tasks_scanned` understates the work"
            );
        }
    }

    assert!(
        unseen.is_empty(),
        "these uninstrumented async boundaries were reported as clean: {unseen:?}. \
         Each one drops the W3C trace context the gate exists to protect, and \
         `std::thread::spawn` is live in this repository today."
    );
    assert!(
        misaccused.is_empty(),
        "these spawns already carry a span and must not be reported as \
         detached: {misaccused:?}"
    );
}

#[test]
fn the_uninstrumented_thread_spawns_living_in_this_repository_are_seen() {
    // Not a synthetic fixture: the hunk is cut out of the real file at run
    // time, so it tracks the source instead of drifting away from it.
    const LIVE_FILE: &str = "src/predictive_test_selector/workspace_dag.rs";
    let source = std::fs::read_to_string(LIVE_FILE)
        .unwrap_or_else(|e| panic!("{LIVE_FILE} must be readable to build this fixture: {e}"));

    let lines: Vec<&str> = source.lines().collect();
    let spawn_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.contains("std::thread::spawn"))
        .map(|(i, _)| i)
        .collect();

    assert!(
        spawn_lines.len() >= 2,
        "fixture drawn from live source has rotted: {LIVE_FILE} no longer \
         contains at least two `std::thread::spawn` calls (found {}). Re-cut \
         this fixture from a file that still spawns uninstrumented threads, or \
         drop it if none remain.",
        spawn_lines.len()
    );

    let start = spawn_lines[0].saturating_sub(2);
    let end = (spawn_lines[spawn_lines.len() - 1] + 8).min(lines.len());
    let hunk = as_added(&lines[start..end].join("\n"));

    let report = run(&diff_of(&[(LIVE_FILE, &hunk)]));

    assert!(
        report.detached_findings.len() >= 2,
        "{LIVE_FILE} spawns {} reader threads with no span attached, and the \
         gate reported {} finding(s). Summary was: {}",
        spawn_lines.len(),
        report.detached_findings.len(),
        report.summary
    );
    assert!(
        report.tasks_scanned >= 2,
        "the two live thread spawns must be counted among the boundaries \
         inspected; `tasks_scanned` was {}",
        report.tasks_scanned
    );
    assert!(
        !report.is_propagated,
        "a file that drops trace context across two real thread boundaries \
         must not be reported as propagating it"
    );
    assert!(
        report
            .detached_findings
            .iter()
            .any(|f| f.file_path.contains("workspace_dag.rs")),
        "a finding must name the file it is in so a reviewer can go to it; \
         got {:?}",
        report
            .detached_findings
            .iter()
            .map(|f| f.file_path.clone())
            .collect::<Vec<_>>()
    );
}

// -------------------------------------------------------------------------
// 3. Only lines this pull request added or kept were inspected
// -------------------------------------------------------------------------

#[test]
fn a_pull_request_that_only_deletes_spawns_inspected_nothing_and_is_accused_of_nothing() {
    let report = run(&diff_of(&[(
        "src/worker.rs",
        "-    tokio::spawn(async move {\n-        work().await;\n-    });\n+    // the background worker was removed",
    )]));

    assert_eq!(
        report.tasks_scanned, 0,
        "a removed line is not code this pull request ships, so it was not an \
         async boundary the gate inspected; summary was: {}",
        report.summary
    );
    assert!(
        report.detached_findings.is_empty(),
        "deleting an uninstrumented spawn removes the defect; the gate reported \
         {} finding(s) against the author for doing so",
        report.detached_findings.len()
    );
    assert!(
        verification_claims_in(&report.summary).is_empty(),
        "having inspected nothing, the gate may not claim a verification; it \
         published {:?} in: {}",
        verification_claims_in(&report.summary),
        report.summary
    );
}

#[test]
fn added_and_retained_lines_are_counted_but_removed_ones_are_not() {
    // One addition, one unchanged context line, one removal -- two boundaries
    // exist in the merged file, not three.
    let report = run(&diff_of(&[(
        "src/worker.rs",
        "+    tokio::spawn(async move { added().await; }.instrument(span()));\n     tokio::spawn(async move { kept().await; }.instrument(span()));\n-    tokio::spawn(async move { removed().await; }.instrument(span()));",
    )]));

    assert_eq!(
        report.tasks_scanned, 2,
        "the merged file contains two spawns -- one added, one retained. The \
         third is being deleted and is not part of what this pull request \
         ships. Summary was: {}",
        report.summary
    );
}

// -------------------------------------------------------------------------
// 4. The verdict must survive the wiring
// -------------------------------------------------------------------------

#[test]
fn the_evaluator_does_not_collapse_the_trace_verdict_through_a_boolean() {
    // Same idiom, and same reason, as
    // `tests/evaluator_preserves_gate_verdicts_test.rs`: this is a defect in
    // the wiring that no guard-level test can observe. A `bool` has two
    // inhabitants, so rebuilding the status from `is_propagated` discards any
    // third outcome the guard reports -- the honest sentence would reach the
    // pull request while the scorecard column stayed a bare Passed/Failed.
    let src = std::fs::read_to_string("src/pre_merge_guard/evaluator.rs")
        .expect("evaluator.rs must exist");
    let production: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !production.contains("= if trace_report.is_"),
        "the evaluator rebuilds `trace_status` from a boolean on \
         `TraceContextReport`, which can carry only two outcomes; whatever the \
         gate reports when it measured nothing is destroyed here before it \
         reaches the scorecard"
    );
}
