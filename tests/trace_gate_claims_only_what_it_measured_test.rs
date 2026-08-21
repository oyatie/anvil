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
//! The sentence is pinned in both directions, because a gate can lie either
//! way. Forbidding the word *verified* on an empty measurement, and nothing
//! else, is satisfied by a constant string -- `"nothing to report"` published on
//! every path, including onto a pull request carrying two uninstrumented thread
//! spawns. So the sentence is pinned where the gate *did* measure too: it must
//! name the defect it found (`the_uninstrumented_thread_spawns_...`), it must
//! differ from the sentence published when nothing was inspected, and it must
//! state the count it inspected (`every_form_of_task_spawn_...`). No phrasing is
//! required beyond the word lists below.
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
//! under either resolution. `is_propagated` is asserted *true* only on the case
//! that has no competing precedent at all: a diff that was measured, every
//! boundary in which carries a span. That one passes under either option, and
//! leaving it unpinned lets `is_propagated: false` ship hardcoded -- detection
//! working perfectly underneath, and every pull request in the repository
//! failed by gate 17 forever.
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
//! Widening the match is where a gate starts accusing the innocent, so the table
//! pins the other side too. `line.contains("spawn")` satisfies every positive
//! row above; it also flags `Command::new("cargo").spawn()?` -- a child process,
//! not a traced task, which will never carry `.instrument(...)` -- along with a
//! spawn named in a comment and one inside a string literal. Those three rows
//! must produce no finding *and no count*: a line that is not an async boundary
//! is not a boundary the gate inspected, and counting it inflates the very
//! number section 1 forbids the gate to overstate.
//!
//! The lookahead is pinned as a behaviour rather than as a constant. Every
//! instrumented fixture in the first draft of this file put `.instrument(` two
//! lines below its spawn, which pinned nothing beyond "the window reaches three
//! lines" and left `span_tracker.rs:36`'s `idx + 6` untouched -- so a correctly
//! instrumented spawn with an ordinary multi-line body is reported as detached,
//! and the gate blocks a merge over a defect the author did not commit. One row
//! now carries its `.instrument(...)` nine lines below the spawn.
//!
//! In the other direction the counter is inflated: `tasks_scanned` increments
//! for every line containing the literal, `-` removal lines included, so a pull
//! request that *deletes* spawns reports having inspected them -- and, worse,
//! `scan_detached_tasks` reads the same removal lines and accuses the author of
//! introducing the very thing they removed.
//!
//! Classifying a diff line by its prefix is where the obvious fix panics.
//! `&line[1..]` is out of range on an empty line, and `split_at(1)` panics on
//! any line whose first character is multi-byte -- and this corpus carries
//! Korean in four Rust files, `src/compliance_guard/statutes.rs:5` among them.
//! The gate reads text handed to it by a webhook, so a panic here takes the
//! whole evaluation down with it. One fixture therefore carries a wholly empty
//! line, a Korean comment lifted from that file, and a line with no diff prefix
//! at all. `run()` unwraps, so a panic is a hard red.
//!
//! ## Out of scope -- stated, rather than left ambiguous
//!
//! - The `.rs`-substring chunk filter is crude in both directions (a Markdown
//!   file that mentions `.rs` is scanned; a Rust file renamed in a chunk that
//!   does not is skipped) but was not in this lane's verified scope, and nothing
//!   here asserts about it.
//! - A call whose callee is the bare identifier `spawn` (`use tokio::task::spawn;`
//!   then `spawn(async move { .. })`) or an alias of it (`use tokio::task::spawn
//!   as go;` then `go(..)`) is **not pinned in either direction**: flagging it
//!   and ignoring it both pass this suite. The word alone is not evidence -- the
//!   `Command::new("cargo").spawn()?` row is the proof -- and telling one from
//!   the other needs the use-list or the receiver's type, which a line scanner
//!   does not have. Named here as a known limit of the gate, not overlooked.
//! - The mapping at `src/pre_merge_guard/evaluator.rs:292` is not pinned here,
//!   and the reason is section 2. An earlier draft asserted that the file does
//!   not contain the substring `= if trace_report.is_`. With no report field
//!   added, `TraceContextReport` carries only a `bool`, so there is no third
//!   outcome for the evaluator to read and the only way to green that assertion
//!   was to rename a local or swap `if` for `match`: it certified a reformat,
//!   which is the false assurance this lane exists to remove. What that line
//!   must become is downstream of the open question -- under `NotMeasured` the
//!   report must carry a status and the mapping must read it; under `Passed` the
//!   boolean is behaviourally adequate and the line is already correct. It is
//!   filed as the second half of that decision, to be pinned once it is made.
//!
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

/// The same hunk as it arrives from a CRLF-authored file: every body line keeps
/// a trailing `\r` that the diff's own `\n` separators do not have.
fn as_crlf(body: &str) -> String {
    body.lines()
        .map(|l| format!("{l}\r"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The sentence the gate publishes when it inspected nothing at all, used as a
/// baseline. A summary produced for a diff that *was* measured must not be this
/// same string, or the gate is telling every author the same thing regardless
/// of what it found.
///
/// The path is the one the table rows use, deliberately. Against a baseline cut
/// from some other file, a summary that merely quotes the file name would
/// differ from it for a reason that has nothing to do with the measurement, and
/// the comparison would pass while saying nothing.
fn summary_when_nothing_was_inspected(path: &str) -> String {
    run(&diff_of(&[(path, "+pub const ROWS: usize = 3;")])).summary
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
// 2. Every form of task spawn is an async boundary -- and nothing else is
// -------------------------------------------------------------------------

/// What a fixture is. Three outcomes rather than two, because "produces no
/// finding" covers two situations that must not be allowed to look alike: an
/// instrumented spawn *was* inspected, so it is counted and the summary carries
/// the count, while a `Command::spawn()` was never an async boundary, so
/// counting it inflates the number section 1 forbids the gate to overstate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Expect {
    /// A real async boundary with no span attached: reported, and counted.
    Detached,
    /// A real async boundary carrying its span: not reported, still counted.
    InstrumentedBoundary,
    /// Not an async boundary at all: not reported, and not counted either.
    NotABoundary,
}

#[test]
fn every_form_of_task_spawn_in_use_here_is_inspected_and_an_instrumented_one_is_clean() {
    use Expect::*;

    let cases: &[(&str, &str, Expect)] = &[
        (
            "tokio::spawn",
            "pub async fn dispatch() {\n    tokio::spawn(async move {\n        work().await;\n    });\n}",
            Detached,
        ),
        (
            "tokio::task::spawn",
            "pub async fn dispatch() {\n    tokio::task::spawn(async move {\n        work().await;\n    });\n}",
            Detached,
        ),
        (
            "tokio::task::spawn_blocking",
            "pub async fn dispatch() {\n    tokio::task::spawn_blocking(move || {\n        heavy_work();\n    });\n}",
            Detached,
        ),
        (
            "JoinSet::spawn",
            "pub async fn dispatch() {\n    let mut set = tokio::task::JoinSet::new();\n    set.spawn(async move {\n        work().await;\n    });\n}",
            Detached,
        ),
        (
            "std::thread::spawn",
            "pub fn dispatch() {\n    let handle = std::thread::spawn(move || {\n        drain_pipe();\n    });\n    let _ = handle.join();\n}",
            Detached,
        ),
        (
            "tokio::spawn carrying a span",
            "pub async fn dispatch() {\n    tokio::spawn(async move {\n        work().await;\n    }.instrument(tracing::info_span!(\"worker\")));\n}",
            InstrumentedBoundary,
        ),
        (
            "tokio::task::spawn carrying a span",
            "pub async fn dispatch() {\n    tokio::task::spawn(async move {\n        work().await;\n    }.instrument(tracing::info_span!(\"worker\")));\n}",
            InstrumentedBoundary,
        ),
        (
            // The ordinary shape of real code: the span is attached below a
            // body nine lines long. A five-line lookahead calls this detached
            // and blocks the merge over a defect that is not there.
            "a span attached below a body longer than any fixed window",
            "pub async fn dispatch() {\n    tokio::spawn(\n        async move {\n            let a = load().await;\n            let b = transform(a).await;\n            let c = enrich(b).await;\n            let d = validate(c).await;\n            let e = persist(d).await;\n            publish(e).await;\n        }\n        .instrument(tracing::info_span!(\"worker\")),\n    );\n}",
            InstrumentedBoundary,
        ),
        (
            // A child process is not a traced task and will never carry
            // `.instrument(...)`. This line is live at
            // `src/predictive_test_selector/workspace_dag.rs:28`, so a matcher
            // that reads the word alone fails every pull request touching it.
            "std::process::Command::spawn",
            "pub fn build() -> std::io::Result<()> {\n    let mut child = std::process::Command::new(\"cargo\").spawn()?;\n    let _ = child.wait()?;\n    Ok(())\n}",
            NotABoundary,
        ),
        (
            "a spawn named in a comment",
            "pub async fn dispatch() {\n    // tokio::spawn(async move { work().await; });\n    work().await;\n}",
            NotABoundary,
        ),
        (
            // Exactly what `span_tracker.rs` carries in its own source.
            "a spawn inside a string literal",
            "pub const SPAWN_NEEDLE: &str = \"tokio::spawn(\";",
            NotABoundary,
        ),
    ];

    const FIXTURE_PATH: &str = "src/dispatch.rs";
    let nothing_inspected = summary_when_nothing_was_inspected(FIXTURE_PATH);
    let mut unseen = Vec::new();
    let mut misaccused = Vec::new();
    // Every row is exercised before anything is asserted, so one wrong row does
    // not hide the rest: the failure names all of them at once.
    let mut misreported: Vec<String> = Vec::new();

    for (label, code, expect) in cases {
        let report = run(&diff_of(&[(FIXTURE_PATH, &as_added(code))]));
        let found = !report.detached_findings.is_empty();

        match (*expect, found) {
            (Detached, false) => unseen.push(*label),
            (InstrumentedBoundary, true) | (NotABoundary, true) => misaccused.push(*label),
            _ => {}
        }

        if *expect == NotABoundary {
            if report.tasks_scanned != 0 {
                misreported.push(format!(
                    "{label}: not an async boundary, so the gate inspected none \
                     here, but `tasks_scanned` was {} -- counting it inflates \
                     the number the summary publishes. Summary was: {}",
                    report.tasks_scanned, report.summary
                ));
            }
            continue;
        }

        // Everything below concerns a diff the gate really did measure.
        if report.tasks_scanned < 1 {
            misreported.push(format!(
                "{label}: a real async boundary was in this diff and \
                 `tasks_scanned` was {}, so the count does not reflect the work \
                 the gate actually did",
                report.tasks_scanned
            ));
        }
        if report.summary == nothing_inspected {
            misreported.push(format!(
                "{label}: the gate measured a boundary here and published the \
                 very sentence it publishes for a diff it never looked at, so \
                 the sentence carries no information about what happened: {}",
                report.summary
            ));
        }

        if *expect == InstrumentedBoundary {
            if !report.is_propagated {
                misreported.push(format!(
                    "{label}: every boundary in this diff carries a span and the \
                     gate measured them, so trace context is propagated. \
                     Reporting otherwise fails a pull request that did nothing \
                     wrong. Summary was: {}",
                    report.summary
                ));
            }
            if !report.summary.contains(&report.tasks_scanned.to_string()) {
                misreported.push(format!(
                    "{label}: the sentence must state how many boundaries were \
                     inspected ({}), or a clean verdict is once again a claim \
                     with no measurement attached to it. Summary was: {}",
                    report.tasks_scanned, report.summary
                ));
            }
        }
    }

    // One assertion over all three failure modes, so that a scanner which is
    // blind to a spawn form does not hide a scanner which accuses an innocent
    // line: the failure lists every row that went wrong, in every way.
    let mut verdict = String::new();
    if !unseen.is_empty() {
        verdict.push_str(&format!(
            "\nreported as clean, but each is an uninstrumented async boundary \
             that drops the W3C trace context this gate exists to protect \
             (`std::thread::spawn` is live in this repository today): {unseen:?}"
        ));
    }
    if !misaccused.is_empty() {
        verdict.push_str(&format!(
            "\nreported as detached, but none of these is a detached async \
             boundary -- either it carries a span, or it was never a traced \
             task at all: {misaccused:?}"
        ));
    }
    if !misreported.is_empty() {
        verdict.push_str(&format!(
            "\nthe finding set was right and the account published of it was \
             not:\n  - {}",
            misreported.join("\n  - ")
        ));
    }
    assert!(verdict.is_empty(), "{}", verdict);
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
        !accusations_in(&report.summary).is_empty(),
        "two real detached boundaries were measured here, so the sentence a \
         reviewer reads must name them. A summary that reports an absence -- or \
         says nothing at all -- conceals a defect the gate did find, which is \
         the same false assurance as claiming a verification it did not \
         perform, pointed the other way. Summary was: {}",
        report.summary
    );

    // A finding is a locator or it is nothing: the file, the line, and the code
    // that was flagged. Two findings against a file that spawns two threads
    // eighty lines apart, both reporting line 0 with an empty snippet, are
    // unactionable.
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
    let mut reported_lines: Vec<usize> = report
        .detached_findings
        .iter()
        .map(|f| f.line_number)
        .collect();
    reported_lines.sort_unstable();
    reported_lines.dedup();
    assert!(
        reported_lines.len() >= 2,
        "the two spawns are at different places in the file, so their findings \
         must carry different line numbers; got {:?}",
        report
            .detached_findings
            .iter()
            .map(|f| f.line_number)
            .collect::<Vec<_>>()
    );
    assert!(
        report
            .detached_findings
            .iter()
            .all(|f| f.snippet.contains("std::thread::spawn")),
        "each finding must quote the line it is about, so a reviewer can see \
         what was flagged without opening the file; got {:?}",
        report
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>()
    );
}

// -------------------------------------------------------------------------
// 3. Only lines this pull request added or kept were inspected
// -------------------------------------------------------------------------

#[test]
fn a_pull_request_that_only_deletes_spawns_inspected_nothing_and_is_accused_of_nothing() {
    // The hunk also carries the shapes that make a prefix classifier panic when
    // it is written by byte-slicing: a wholly empty line (`&line[1..]` is out of
    // range), a Korean comment lifted from `src/compliance_guard/statutes.rs:5`,
    // and a line with no diff prefix at all whose first character is multi-byte
    // (`split_at(1)` panics on it). That last shape is not decoration: the gate
    // is handed webhook text it does not control, and
    // `SpanTracker::scan_detached_tasks` is called with raw unprefixed source in
    // its own unit tests. A panic here is a fail-open for the whole evaluation,
    // and `run()` unwraps, so it is a hard red.
    let body = "-    tokio::spawn(async move {\n\
                -        work().await;\n\
                -    });\n\
                +    // the background worker was removed\n\
                \n\
                +    // Pipa,             // Personal Information Protection Act (개인정보 보호법)\n\
                개인정보 보호법 §24의2";

    let report = run(&diff_of(&[("src/worker.rs", body)]));

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
    let body = "+    tokio::spawn(async move { added().await; }.instrument(span()));\n     tokio::spawn(async move { kept().await; }.instrument(span()));\n-    tokio::spawn(async move { removed().await; }.instrument(span()));";
    let report = run(&diff_of(&[("src/worker.rs", body)]));

    assert_eq!(
        report.tasks_scanned, 2,
        "the merged file contains two spawns -- one added, one retained. The \
         third is being deleted and is not part of what this pull request \
         ships. Summary was: {}",
        report.summary
    );

    // The identical hunk out of a CRLF-authored file. Every body line now ends
    // in `\r`, so a classifier that matches or anchors at end-of-line -- an
    // `.instrument` regex closed with `$`, a `line.ends_with(");")` -- quietly
    // reclassifies all three lines and counts or accuses wrongly.
    let crlf = run(&diff_of(&[("src/worker.rs", &as_crlf(body))]));

    assert_eq!(
        crlf.tasks_scanned, report.tasks_scanned,
        "the line endings of the file an author happens to work in are not a \
         property of their code; the CRLF hunk counted {} boundaries against \
         {} for the identical LF hunk. Summary was: {}",
        crlf.tasks_scanned, report.tasks_scanned, crlf.summary
    );
    assert_eq!(
        crlf.detached_findings.len(),
        report.detached_findings.len(),
        "every spawn in this hunk carries a span under either line ending; the \
         CRLF hunk produced {} finding(s) against {}. Summary was: {}",
        crlf.detached_findings.len(),
        report.detached_findings.len(),
        crlf.summary
    );
}
