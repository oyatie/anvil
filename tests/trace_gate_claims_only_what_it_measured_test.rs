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
//! publish the number it measured -- the count of boundaries inspected on a
//! clean diff (`every_form_of_task_spawn_...`), the count of detached ones on a
//! failing diff (`the_uninstrumented_thread_spawns_...`).
//!
//! That number is read as a *token*, not as a substring. `summary.contains("1")`
//! is satisfied by the `1` in `Gate 17`, so the constant sentence "Gate 17: trace
//! context reviewed" clears a substring check while reporting nothing about what
//! happened. `numeric_tokens` splits the sentence on non-digits and requires the
//! exact number among the results, and the clean case is exercised at a count of
//! three so no incidental digit can supply it. No phrasing is required beyond
//! that number and the word lists below.
//!
//! ## 2. The status for the nothing-in-scope case is deliberately NOT pinned
//!
//! This repository contains two settled precedents that disagree, and this lane
//! is not entitled to pick between them by writing a test:
//!
//!   - `src/slo_canary_guard/mod.rs:153` -- no telemetry source, so
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
//! One invariant *is* pinned across both nothing-in-scope routes, and it is the
//! one that keeps the question open rather than answering it:
//! `is_propagated || !detached_findings.is_empty()` -- a gate may not publish a
//! blocking verdict against a diff it produced no finding against. `Passed`
//! satisfies it. A future `NotMeasured` carried in a report field satisfies it,
//! because that route does not mean "unmeasured" by setting the boolean false.
//! What fails it is the one resolution that is not a resolution: encoding "not
//! measured" as `is_propagated = false` because no report field was added.
//! `evaluator.rs:292` reads that boolean and nothing else, so it publishes
//! `Failed`, and gate 17 becomes a permanent block on every documentation-only
//! pull request and every spawn-free Rust one -- the open question shipped as a
//! merge block, with the suite green.
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
//! A fourth exclusion is cut from live source rather than written out, because
//! it is the exact mirror of the `std::thread::spawn` false negative and the
//! obvious widening walks into it. `\bspawn\w*\s*\(` -- the shortest way to
//! pick up `spawn_blocking` -- also matches
//! `self_governor.spawn_monitoring_daemon()` and
//! `.spawn_continuous_poller(..)`, both live in `src/cli/server.rs`. Neither is
//! a task boundary and neither will ever carry `.instrument(...)`, so that
//! widening fails every pull request touching that file. The second takes
//! arguments, so the empty-parens rule that saves `Command::spawn()` does not
//! save it.
//!
//! Each of these rows is shaped so the shortest fix does not reach it. The
//! commented spawn sits *after* real code on its line, so a rule that tests the
//! line's first characters still flags a trailing `// tokio::spawn(..)` and
//! blocks a merge over a defect the author did not commit; only truncating the
//! line at `//` passes it alongside the positive rows. And the two
//! `JoinSet::spawn` rows spell their receiver differently -- `workers.spawn(..)`
//! detached, `set.spawn(..)` instrumented -- so no allowlist of the exact
//! spellings that appear in this file covers both, and a receiver-method spawn
//! has to be recognised as a form rather than as a literal. Recognised as a
//! literal, the row it misses is not flagged and not counted either: an
//! uninstrumented boundary published as "nothing was inspected", which is
//! section 1's defect wearing the other face.
//!
//! Where a finding *points* is pinned by the same reasoning. One row of
//! `a_file_that_instruments_one_spawn_and_forgets_the_other_...` runs inside a
//! diff of three files, with a spawn-free one on each side of the file under
//! test. In a single-file diff a finding's `file_path` needs no relationship to
//! the chunk the flagged line was in -- the first changed file, the last chunk
//! header, and the constant `"src/dispatch.rs"` are all indistinguishable from
//! per-chunk attribution -- and all of them send a reviewer to a file with no
//! spawn in it as soon as a pull request touches more than one.
//!
//! The lookahead is pinned as a behaviour rather than as a constant. Every
//! instrumented fixture in the first draft of this file put `.instrument(` two
//! lines below its spawn, which pinned nothing beyond "the window reaches three
//! lines" and left `span_tracker.rs:36`'s `idx + 6` untouched -- so a correctly
//! instrumented spawn with an ordinary multi-line body is reported as detached,
//! and the gate blocks a merge over a defect the author did not commit. One row
//! now carries its `.instrument(...)` nine lines below the spawn.
//!
//! A window that is merely *wide* is not the behaviour either. The gate's whole
//! job is deciding which boundary carries a span, and a diff whose every spawn is
//! instrumented -- or whose every spawn is not -- cannot tell that apart from
//! `content.contains(".instrument(")` asked once per chunk. So one file here
//! carries an instrumented spawn and a detached one together
//! (`a_file_that_instruments_one_spawn_and_forgets_the_other_...`): exactly one
//! finding, quoting the detached call and not the instrumented one. A second such
//! file adds `Command::new(..).spawn()?` beside them and pins the count exactly,
//! so a non-boundary cannot be counted merely because the file it sits in does
//! contain real ones.
//!
//! Order matters as much as width, and pinning only width leaves the gate's
//! subject unpinned. A fixture that always places the instrumented spawn first
//! constrains how *far* a forward lookahead reaches and never what it is allowed
//! to reach: a window of any width -- including an unbounded one -- clears a
//! detached boundary with the span belonging to the *next* spawn below it, and
//! publishes "each attaches a tracing span" over a real uninstrumented boundary
//! that was counted and never measured. So the same two boundaries are pinned in
//! both orderings. In the detached-first file the instrumented spawn sits close
//! enough below that no narrowing rescues an implementation whose window does not
//! stop at the boundary it is about.
//!
//! The detached side of the multi-line call form is pinned too
//! (`a_multi_line_spawn_with_no_span_...`). Widening the window and *suppressing*
//! the multi-line form -- "the call opens on its own line, that shape is handled
//! elsewhere" -- both green an all-instrumented long-body fixture, and the second
//! publishes a clean verdict over the ordinary shape of any spawn with a real
//! body, which is the defect this lane exists to remove. The finding must quote
//! the line that opens the call, and its line number must move with that line
//! when the fixture is padded above.
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
//! whole evaluation down with it.
//!
//! Placing those shapes *beside* a spawn does not reach the panic, and a fixture
//! that does only that is a fixture that passes whether or not the code is
//! correct. The natural optimisation is a `line.contains("spawn")` pre-filter in
//! front of the classifier, and no hazardous line in an ordinary fixture says
//! `spawn`, so the byte-slicing ships intact behind an accident of fixture
//! composition. They are therefore placed where a scan has to read them: inside
//! the body of a spawn whose span is attached below it, which a lookahead must
//! walk end to end. One further line has no diff prefix at all, begins with a
//! multi-byte character, *and* carries a spawn call, so no pre-filter can route
//! around the classifier -- the raw-source shape
//! `SpanTracker::scan_detached_tasks` is already called with in its own unit
//! tests. That line attaches its own span, so it pins the panic without pinning
//! whether an unprefixed line counts as a line the pull request ships. `run()`
//! unwraps, so a panic is a hard red.
//!
//! ## Out of scope -- stated, rather than left ambiguous
//!
//! - The `.rs` chunk filter is pinned in one direction and one only. The
//!   documentation-only row carries a fenced Rust example with an uninstrumented
//!   spawn in it, so a scanner that reads every chunk regardless of the path it
//!   belongs to produces a finding, and gate 17 becomes a permanent block on any
//!   documentation change that shows an uninstrumented spawn. That direction
//!   bites. The other does not, and cannot with this helper: `diff_of` always
//!   emits `--- a/{path}` and `+++ b/{path}`, so a `.rs` path always puts `.rs`
//!   into its own chunk text and a `.md` path never does. No fixture here can
//!   separate "the path was consulted" from "the chunk text was searched", and
//!   nothing here claims to. The filter stays crude both ways -- a Markdown file
//!   that merely mentions `.rs` is scanned, a Rust file renamed in a chunk that
//!   does not mention it is skipped -- and neither of those was in this lane's
//!   verified scope.
//! - Only `//` line comments are handled. A spawn inside a `/* ... */` block
//!   comment is not pinned in either direction: flagging it and ignoring it both
//!   pass this suite. Named as a known limit of the gate rather than left
//!   silent.
//! - A `.instrument(` belonging to some *other* call inside a spawn's own region
//!   still clears that spawn, so
//!   `tokio::spawn(async move { foo(x.instrument(sp)).await; });` reads clean
//!   here. What is pinned is that a span belongs to the boundary that opened it
//!   rather than to the next one down; attribution finer than that is not.
//! - Whether `line_number` counts from the file, the chunk or the hunk is not
//!   pinned. `a_multi_line_spawn_...` pins only that the number moves with the
//!   code it locates, and `the_uninstrumented_thread_spawns_...` only that two
//!   findings in different places carry different numbers. Chunk-relative
//!   numbering -- what ships today, off by the header count -- passes.
//! - The summary and the `issue` field are pinned for honesty, not for
//!   usefulness, and that is the ceiling of reading them by word list and
//!   numeric token rather than an oversight. A gate publishing `"none"`,
//!   `"3 inspected"`, `"failed 1"` and an `issue` of `"span"` clears every
//!   assertion in this file: right verdict, right count, sentences that teach
//!   very little. Tightening that means pinning phrasing, which pins one
//!   implementation's wording in place of a behaviour.
//! - The CRLF half of `added_and_retained_lines_are_counted_but_removed_ones_are_not`
//!   reaches less than a reader would assume, and it says so rather than
//!   claiming otherwise. `str::lines()` strips a trailing `\r` before any matcher
//!   sees it, and a snippet built with `.trim()` -- which the shipping code
//!   already does -- strips it again. So against any `.lines()`-based *or*
//!   trimming scanner every assertion in that block holds unconditionally,
//!   whether or not its author ever thought about line endings; an earlier draft
//!   of this file claimed the carriage-return assertion reached every
//!   implementation, and it does not. What the block does bite is one plausible
//!   pair: a scanner that splits the diff on `'\n'` itself *and* publishes the
//!   raw line, where an end-anchored matcher or a `line.ends_with(");")`
//!   reclassifies the hunk and the located set stops matching the LF run's. It is
//!   kept for that pair and for no wider claim, and it costs one comparison.
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

/// Cuts a window of live source ending at the first line containing `needle`,
/// together with `lead` lines above it for context. A fixture that is meant to
/// be a statement about this repository has to be lifted out of it at test time,
/// or it stops being one the moment the source moves.
fn live_lines_ending_at(path: &str, needle: &str, lead: usize) -> String {
    let source = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{path} must be readable to build this fixture: {e}"));
    let lines: Vec<&str> = source.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| {
            panic!(
                "fixture drawn from live source has rotted: {path} no longer \
                 contains a line matching {needle:?}. Re-cut this fixture from a \
                 live call site, or drop the row if none remains."
            )
        });
    lines[at.saturating_sub(lead)..=at].join("\n")
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

/// Every whole number the summary states, as a token rather than as a substring.
///
/// `summary.contains(&n.to_string())` is not a pin on a reported count: the `1`
/// in "Gate 17" satisfies it for `n == 1`, so a constant sentence that reports
/// nothing passes. Splitting on non-digits and requiring the exact number among
/// the results means the sentence has to actually carry it.
fn numeric_tokens(summary: &str) -> Vec<usize> {
    summary
        .split(|c: char| !c.is_ascii_digit())
        .filter(|t| !t.is_empty())
        .filter_map(|t| t.parse::<usize>().ok())
        .collect()
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
fn a_gate_that_inspected_nothing_says_so_plainly_and_accuses_no_one() {
    // Both routes into an empty measurement, and every obligation that follows
    // from one, in a single loop. The first route is the chunk filter, which is
    // the case issue #14 files. The second is the one it understates and the one
    // that fires far more often: a diff that is entirely Rust, so the filter lets
    // it through, and that spawns no task, so the scan finds nothing to look at.
    //
    // This test is deliberately silent on which GateStatus the nothing-in-scope
    // case deserves: it passes whether the owner chooses NotMeasured (the
    // slo_canary_guard precedent) or Passed (the coverage_guard precedent),
    // because both precedents publish a sentence satisfying every condition
    // below. What it does pin is what the two precedents agree on, which is not
    // a choice to make.
    for (label, files) in [
        (
            // The body carries a fenced Rust example with an uninstrumented
            // spawn in it. Without that, this row is the spawn-free Rust row
            // below it written twice: every assertion in the loop holds for one
            // exactly when it holds for the other, and neither reaches the
            // chunk filter this row is nominally about. With it, a scanner that
            // reads every chunk regardless of the path it belongs to finds the
            // spawn, produces a finding, and publishes a permanent merge block
            // on any documentation change that shows an uninstrumented spawn --
            // the accusation the loop's own assertion forbids.
            "a documentation-only pull request",
            &[(
                "docs/adr/0002-honesty.md",
                "+The published name must match the live measurement. For example,\n+this is not a boundary in this pull request, it is prose about one:\n+\n+```rust\n+tokio::spawn(async move { work().await; });\n+```",
            )][..],
        ),
        (
            "a Rust pull request that spawns nothing",
            &[(
                "src/compute.rs",
                "+pub fn compute(rows: &[u32]) -> u32 {\n+    rows.iter().sum()\n+}",
            )][..],
        ),
    ] {
        let report = run(&diff_of(files));

        assert_eq!(
            report.tasks_scanned, 0,
            "{label}: this diff crosses no async boundary, so there was nothing \
             for the gate to inspect. Summary was: {}",
            report.summary
        );
        assert!(
            verification_claims_in(&report.summary).is_empty(),
            "{label}: nothing was inspected, so the gate holds no evidence to \
             claim; it published {:?} in: {}",
            verification_claims_in(&report.summary),
            report.summary
        );
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
        // Which status this case deserves stays open. What is closed here is the
        // route by which the open question ships as a permanent merge block.
        // With no report field added, `is_propagated` is the only channel
        // `src/pre_merge_guard/evaluator.rs:292` reads, so encoding "not
        // measured" as `false` -- the one-token change
        // `detached_findings.is_empty() && tasks_scanned > 0` -- publishes
        // GateStatus::Failed against every diff that inspected nothing, while
        // every sentence-level assertion above stays honest and green. Passed
        // satisfies this assertion; a NotMeasured carried in its own field
        // satisfies it; only the boolean overload does not.
        assert!(
            report.is_propagated || !report.detached_findings.is_empty(),
            "{label}: the gate produced no finding against this diff and still \
             reported it as not propagating trace context, which \
             `evaluator.rs:292` publishes as a failed gate. A verdict that \
             blocks a merge has to name what it blocks it for. Summary was: {}",
            report.summary
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

    // Cut from live source rather than written out, because this row is the
    // mirror of the `std::thread::spawn` false negative and the shortest fix for
    // that one walks straight into it. Widening the callee match to
    // `\bspawn\w*\s*\(` picks up `spawn_blocking` and also picks up these two,
    // which are ordinary methods that merely start with `spawn_`. Neither is a
    // task boundary; neither will ever carry `.instrument(...)`; so that
    // widening fails every pull request touching `src/cli/server.rs`. The second
    // of the two matters more than the first, because it takes arguments and the
    // empty-parens rule that saves `Command::spawn()` does not save it.
    const LIVE_CALLERS: &str = "src/cli/server.rs";
    let spawn_prefixed_methods = format!(
        "{}\n{}",
        live_lines_ending_at(LIVE_CALLERS, "spawn_monitoring_daemon", 1),
        live_lines_ending_at(LIVE_CALLERS, "spawn_continuous_poller", 2),
    );

    // The final column is the number of async boundaries the gate must report
    // having inspected in that fixture, pinned exactly rather than as `>= 1`:
    // a count that is merely non-zero says nothing about whether the gate
    // counted the boundaries or counted the lines that mention one.
    let cases: &[(&str, &str, Expect, usize)] = &[
        (
            "tokio::spawn",
            "pub async fn dispatch() {\n    tokio::spawn(async move {\n        work().await;\n    });\n}",
            Detached,
            1,
        ),
        (
            "tokio::task::spawn",
            "pub async fn dispatch() {\n    tokio::task::spawn(async move {\n        work().await;\n    });\n}",
            Detached,
            1,
        ),
        (
            "tokio::task::spawn_blocking",
            "pub async fn dispatch() {\n    tokio::task::spawn_blocking(move || {\n        heavy_work();\n    });\n}",
            Detached,
            1,
        ),
        (
            // Spelled `workers`, while the instrumented JoinSet row further
            // down is spelled `set`. An allowlist of the receiver names that
            // happen to appear in this file covers one row or the other, never
            // both, so a receiver-method spawn has to be recognised as a form.
            // Recognised as a literal instead, the row it misses is reported as
            // nothing-in-scope: an uninstrumented boundary published as
            // "nothing was inspected", which is section 1's defect wearing the
            // other face, in the spawn form this row was added to cover.
            "JoinSet::spawn",
            "pub async fn dispatch() {\n    let mut workers = tokio::task::JoinSet::new();\n    workers.spawn(async move {\n        work().await;\n    });\n}",
            Detached,
            1,
        ),
        (
            "std::thread::spawn",
            "pub fn dispatch() {\n    let handle = std::thread::spawn(move || {\n        drain_pipe();\n    });\n    let _ = handle.join();\n}",
            Detached,
            1,
        ),
        (
            "tokio::spawn carrying a span",
            "pub async fn dispatch() {\n    tokio::spawn(async move {\n        work().await;\n    }.instrument(tracing::info_span!(\"worker\")));\n}",
            InstrumentedBoundary,
            1,
        ),
        (
            "tokio::task::spawn carrying a span",
            "pub async fn dispatch() {\n    tokio::task::spawn(async move {\n        work().await;\n    }.instrument(tracing::info_span!(\"worker\")));\n}",
            InstrumentedBoundary,
            1,
        ),
        (
            // The ordinary shape of real code: the span is attached below a
            // body nine lines long. A five-line lookahead calls this detached
            // and blocks the merge over a defect that is not there.
            "a span attached below a body longer than any fixed window",
            "pub async fn dispatch() {\n    tokio::spawn(\n        async move {\n            let a = load().await;\n            let b = transform(a).await;\n            let c = enrich(b).await;\n            let d = validate(c).await;\n            let e = persist(d).await;\n            publish(e).await;\n        }\n        .instrument(tracing::info_span!(\"worker\")),\n    );\n}",
            InstrumentedBoundary,
            1,
        ),
        (
            // Three, so that the count the summary must publish is a number no
            // stray digit supplies by accident. A sentence reporting "1" or
            // carrying the `17` of "gate 17" does not contain the token `3`.
            "three instrumented boundaries in one file",
            "pub async fn fan_out() {\n    tokio::spawn(async move { alpha().await; }.instrument(tracing::info_span!(\"alpha\")));\n    tokio::task::spawn(async move { beta().await; }.instrument(tracing::info_span!(\"beta\")));\n    let mut set = tokio::task::JoinSet::new();\n    set.spawn(async move { gamma().await; }.instrument(tracing::info_span!(\"gamma\")));\n}",
            InstrumentedBoundary,
            3,
        ),
        (
            // A child process is not a traced task and will never carry
            // `.instrument(...)`. This line is live at
            // `src/predictive_test_selector/workspace_dag.rs:28`, so a matcher
            // that reads the word alone fails every pull request touching it.
            "std::process::Command::spawn",
            "pub fn build() -> std::io::Result<()> {\n    let mut child = std::process::Command::new(\"cargo\").spawn()?;\n    let _ = child.wait()?;\n    Ok(())\n}",
            NotABoundary,
            0,
        ),
        (
            // The comment sits after real code on the line, so this row is not
            // greened by testing the line's first characters -- the shortest
            // fix, and one that still flags a trailing `// tokio::spawn(..)`
            // beside live code, blocking a merge over a defect the author did
            // not commit. Truncating the line at `//` is what passes both this
            // row and every positive row above it.
            "a spawn named in a comment",
            "pub async fn dispatch() {\n    let h = start(); // tokio::spawn(async move { work().await; });\n    let _ = h;\n}",
            NotABoundary,
            0,
        ),
        (
            // Exactly what `span_tracker.rs` carries in its own source.
            "a spawn inside a string literal",
            "pub const SPAWN_NEEDLE: &str = \"tokio::spawn(\";",
            NotABoundary,
            0,
        ),
        (
            "a method whose name merely begins with spawn_, live in this repository",
            spawn_prefixed_methods.as_str(),
            NotABoundary,
            0,
        ),
    ];

    const FIXTURE_PATH: &str = "src/dispatch.rs";
    let nothing_inspected = summary_when_nothing_was_inspected(FIXTURE_PATH);
    let mut unseen = Vec::new();
    let mut misaccused = Vec::new();
    // Every row is exercised before anything is asserted, so one wrong row does
    // not hide the rest: the failure names all of them at once.
    let mut misreported: Vec<String> = Vec::new();

    for (label, code, expect, boundaries) in cases {
        let report = run(&diff_of(&[(FIXTURE_PATH, &as_added(code))]));
        let found = !report.detached_findings.is_empty();

        match (*expect, found) {
            (Detached, false) => unseen.push(*label),
            (InstrumentedBoundary, true) | (NotABoundary, true) => misaccused.push(*label),
            _ => {}
        }

        if report.tasks_scanned != *boundaries {
            misreported.push(format!(
                "{label}: this fixture contains {boundaries} async boundar(ies) \
                 and the gate reported inspecting {}. Under-counting means a \
                 boundary went uninspected under a verdict that covers it; \
                 over-counting inflates the number the summary publishes over a \
                 line that was never a traced task. Summary was: {}",
                report.tasks_scanned, report.summary
            ));
        }

        if *expect == NotABoundary {
            continue;
        }

        // Everything below concerns a diff the gate really did measure.
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
            if !numeric_tokens(&report.summary).contains(boundaries) {
                misreported.push(format!(
                    "{label}: the sentence must state how many boundaries were \
                     inspected ({boundaries}) as a number a reader can find in \
                     it, or a clean verdict is once again a claim with no \
                     measurement attached to it. The numbers it did publish were \
                     {:?}. Summary was: {}",
                    numeric_tokens(&report.summary),
                    report.summary
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
fn a_file_that_instruments_one_spawn_and_forgets_the_other_reports_only_the_forgotten_one() {
    // Deciding *which* boundary carries a span is the gate's entire job, and no
    // homogeneous fixture can pin it. A file whose every spawn is instrumented,
    // and a file whose every spawn is not, are both classified correctly by a
    // single chunk-wide `content.contains(".instrument(")` -- which reports a
    // detached boundary as clean the moment any other line in the file happens
    // to attach a span. Each file below carries both kinds at once, so exactly
    // one finding is the only right answer and it has to name the right call.
    //
    // The second file adds `Command::new(..).spawn()?` beside the two real
    // boundaries. `tasks_scanned` is pinned exactly on both, so a non-boundary
    // cannot be counted merely because the file it sits in does contain real
    // ones -- the case an all-`Command` fixture cannot reach.
    //
    // The third file is the second one's ordering reversed, and it is here
    // because the first two constrain only how far a forward lookahead reaches.
    // With the instrumented spawn always first, a window of any width -- an
    // unbounded one included -- classifies both files correctly while still
    // clearing a detached boundary using the span that belongs to the next
    // spawn below it. That is the same false-assurance sentence this lane
    // exists to remove, pointed at a boundary that was counted and never
    // measured, so the scope of the lookahead is pinned here as well as its
    // width.
    //
    // The last column runs the row inside a diff that also touches two
    // spawn-free files, one on each side of the file under test. With the file
    // under test alone in the diff -- which is how every row in this file used
    // to run -- a finding's `file_path` needs no relationship to the chunk the
    // flagged line was in: `changed_files.first()`, the last chunk's header, or
    // the constant `"src/dispatch.rs"` all satisfy the attribution assertion
    // below, and all three send a reviewer to a file with no spawn in it the
    // moment a pull request touches more than one. `src/before.rs` kills the
    // first, `src/after.rs` the second, and both together kill any attribution
    // that is not per-chunk. Neither neighbour spawns anything, so the counts
    // below are unchanged by their presence.
    const FILE: &str = "src/dispatch.rs";
    const BEFORE: &str = "src/before.rs";
    const AFTER: &str = "src/after.rs";
    const SPAWN_FREE: &str = "pub async fn settle() {\n    ok().await;\n}";
    let cases: &[(&str, &str, usize, bool)] = &[
        (
            // One spelling of spawn throughout, so nothing here is decided by
            // which form was recognised: the instrumented boundary carries an
            // ordinary multi-line body, the detached one does not, and telling
            // them apart is the whole of the gate's job.
            "an instrumented spawn and a detached one, in a diff of three files",
            "pub async fn dispatch() {\n    tokio::spawn(\n        async move {\n            let a = load().await;\n            let b = transform(a).await;\n            let c = enrich(b).await;\n            let d = validate(c).await;\n            let e = persist(d).await;\n            with_span(e).await;\n        }\n        .instrument(tracing::info_span!(\"traced\")),\n    );\n    let _ = settle().await;\n    tokio::spawn(async move { no_span().await; });\n}",
            2,
            true,
        ),
        (
            "the same, with a child process spawned alongside them",
            "pub async fn build_and_dispatch() -> std::io::Result<()> {\n    let mut child = std::process::Command::new(\"cargo\").spawn()?;\n    tokio::task::spawn(async move { with_span().await; }.instrument(tracing::info_span!(\"traced\")));\n    let _ = child.wait()?;\n    tokio::task::spawn_blocking(move || { no_span(); });\n    Ok(())\n}",
            2,
            false,
        ),
        (
            // The same two boundaries, detached first. The instrumented one
            // follows close enough below that no *narrowing* of the window
            // rescues an implementation whose window does not stop at the
            // boundary it is about: the only way to report one finding here is
            // to decide that a span belongs to the spawn that opened it.
            "a detached spawn above an instrumented one",
            "pub async fn dispatch() {\n    tokio::spawn(async move { no_span().await; });\n    let _ = settle().await;\n    tokio::spawn(\n        async move {\n            let a = load().await;\n            with_span(a).await;\n        }\n        .instrument(tracing::info_span!(\"traced\")),\n    );\n}",
            2,
            false,
        ),
    ];

    for (label, code, boundaries, neighbours) in cases {
        let under_test = as_added(code);
        let neighbour = as_added(SPAWN_FREE);
        let files: Vec<(&str, &str)> = if *neighbours {
            vec![
                (BEFORE, neighbour.as_str()),
                (FILE, under_test.as_str()),
                (AFTER, neighbour.as_str()),
            ]
        } else {
            vec![(FILE, under_test.as_str())]
        };
        let report = run(&diff_of(&files));

        assert_eq!(
            report.detached_findings.len(),
            1,
            "{label}: one of the two boundaries in this file attaches a span and \
             the other does not, so exactly one finding is correct. The gate \
             reported {}: {:?}. Zero means a span attached to one boundary was \
             read as covering the other; two means the instrumented one was \
             accused. Summary was: {}",
            report.detached_findings.len(),
            report
                .detached_findings
                .iter()
                .map(|f| f.snippet.clone())
                .collect::<Vec<_>>(),
            report.summary
        );

        let finding = &report.detached_findings[0];
        assert!(
            finding.snippet.contains("no_span") && !finding.snippet.contains("with_span"),
            "{label}: the finding must locate the call that carries no span. It \
             quoted line {} as {:?}, which is the instrumented boundary -- a \
             reviewer sent there finds a span already attached and learns \
             nothing about the real defect.",
            finding.line_number,
            finding.snippet
        );
        assert!(
            finding.file_path.contains("dispatch.rs")
                && !finding.file_path.contains("before.rs")
                && !finding.file_path.contains("after.rs"),
            "{label}: the detached spawn is in {FILE} and the finding named \
             {:?}. A path that does not come from the chunk the flagged line \
             was in sends a reviewer to a file that has no spawn in it at all.",
            finding.file_path
        );
        // The file, the line and the code locate the defect; `issue` is the only
        // field that says what is wrong with the located line, and it is the
        // sentence the author reads next to it. Left empty -- or filled with a
        // restatement of the verdict -- the gate lands on the right line with a
        // message that teaches nothing, which is the same false assurance one
        // field over. No phrasing is required beyond naming the remedy.
        let issue = finding.issue.to_lowercase();
        assert!(
            !issue.trim().is_empty() && (issue.contains("instrument") || issue.contains("span")),
            "{label}: the finding at line {} says {:?}, which does not tell the \
             author what to attach to the boundary it flagged. A locator with no \
             remedy beside it sends a reviewer to the right line and leaves them \
             to guess what the gate wanted.",
            finding.line_number,
            finding.issue
        );
        assert!(
            !report.is_propagated,
            "{label}: a boundary in this diff drops the trace context, so the \
             gate must not report the diff as propagating it. Summary was: {}",
            report.summary
        );
        assert_eq!(
            report.tasks_scanned, *boundaries,
            "{label}: this file contains {boundaries} async boundaries and the \
             gate reported inspecting {}. A `Command::new(..).spawn()?` is a \
             child process, never a traced task, and counting it here would \
             inflate the number the summary publishes just because real \
             boundaries share the file. Summary was: {}",
            report.tasks_scanned, report.summary
        );
    }
}

#[test]
fn a_multi_line_spawn_with_no_span_is_reported_at_the_line_that_opens_it() {
    // The long-body instrumented row of the table above with the
    // `.instrument(...)` removed, in both spellings. That row alone cannot tell
    // "the lookahead window was widened" from "the multi-line call form is
    // suppressed" -- both report it clean, and the second reports *every*
    // multi-line spawn clean, including these two, which is the ordinary shape
    // of any spawn with a real body and precisely the unmeasured boundary this
    // lane exists to stop the gate publishing a clean verdict over.
    const BODY: &str = "pub async fn dispatch() {\n    tokio::spawn(\n        async move {\n            let a = load().await;\n            let b = transform(a).await;\n            let c = enrich(b).await;\n            let d = validate(c).await;\n            let e = persist(d).await;\n            publish(e).await;\n        },\n    );\n    tokio::task::spawn(\n        async move {\n            let a = load().await;\n            let b = transform(a).await;\n            let c = enrich(b).await;\n            let d = validate(c).await;\n            let e = persist(d).await;\n            publish(e).await;\n        },\n    );\n}";
    /// Four ordinary lines, carrying neither a spawn nor a span.
    const PADDING: &str = "use crate::pipeline::load;\nuse crate::pipeline::transform;\nuse crate::pipeline::enrich;\nuse crate::pipeline::publish;";
    const PADDING_LINES: usize = 4;

    let report = run(&diff_of(&[("src/dispatch.rs", &as_added(BODY))]));

    assert_eq!(
        report.detached_findings.len(),
        2,
        "each of these spawns opens on its own line and attaches no span \
         anywhere. The gate reported {} finding(s), so a boundary it never \
         measured is covered by the verdict it published. Findings were {:?}. \
         Summary was: {}",
        report.detached_findings.len(),
        report
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>(),
        report.summary
    );
    assert_eq!(
        report.tasks_scanned, 2,
        "two async boundaries are in this diff. Summary was: {}",
        report.summary
    );
    assert!(
        !report.is_propagated,
        "both boundaries in this diff drop the trace context. Summary was: {}",
        report.summary
    );
    assert!(
        report
            .detached_findings
            .iter()
            .all(|f| f.snippet.contains("spawn(") && !f.snippet.contains("load()")),
        "each finding must quote the line that opens its call, so a reviewer \
         reads the boundary rather than a line of its body; got {:?}",
        report
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>()
    );

    // Where a finding points is pinned by moving the spawn, not by naming a
    // number: nothing here decides whether a line number counts from the file,
    // the chunk or the hunk. Push the same calls four lines down and every
    // number must follow by four. A constant -- the hunk start, the file start,
    // or zero -- does not move.
    let padded = run(&diff_of(&[(
        "src/dispatch.rs",
        &as_added(&format!("{PADDING}\n{BODY}")),
    )]));
    let lines_of = |r: &TraceContextReport| {
        let mut v: Vec<usize> = r.detached_findings.iter().map(|f| f.line_number).collect();
        v.sort_unstable();
        v
    };
    assert_eq!(
        lines_of(&padded),
        lines_of(&report)
            .into_iter()
            .map(|n| n + PADDING_LINES)
            .collect::<Vec<_>>(),
        "the same calls moved four lines down; the reported line numbers went \
         from {:?} to {:?}, so they do not locate the spawns and a reviewer \
         following them lands somewhere else in the file",
        lines_of(&report),
        lines_of(&padded)
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
        !accusations_in(&report.summary).is_empty(),
        "two real detached boundaries were measured here, so the sentence a \
         reviewer reads must name them. A summary that reports an absence -- or \
         says nothing at all -- conceals a defect the gate did find, which is \
         the same false assurance as claiming a verification it did not \
         perform, pointed the other way. Summary was: {}",
        report.summary
    );
    assert!(
        numeric_tokens(&report.summary).contains(&report.detached_findings.len()),
        "the sentence must publish how many boundaries were found detached \
         ({}), as a number a reader can find in it. One of the four words above \
         is not a report -- a bare \"FAILED\" satisfies that and tells the \
         author nothing about the size of what the gate found. The numbers it \
         did publish were {:?}. Summary was: {}",
        report.detached_findings.len(),
        numeric_tokens(&report.summary),
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
    // One thing only: a hunk that removes an uninstrumented spawn and adds a
    // comment in its place. The multi-byte and empty-line shapes that make a
    // byte-slicing prefix classifier panic used to be stapled on here; they now
    // live in `hazardous_lines_inside_a_scanned_body_are_classified_...`, where
    // a scan is obliged to read them rather than merely pass them by.
    let body = "-    tokio::spawn(async move {\n\
                -        work().await;\n\
                -    });\n\
                +    // the background worker was removed";

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
fn hazardous_lines_inside_a_scanned_body_are_classified_rather_than_crashing_the_gate() {
    // A prefix classifier written by byte-slicing panics on two shapes: a wholly
    // empty line (`&line[1..]` is out of range) and a line whose first character
    // is multi-byte (`split_at(1)`). This corpus carries Korean in four Rust
    // files, `src/compliance_guard/statutes.rs:5` among them, and the gate is
    // handed webhook text it does not control, so a panic here takes the whole
    // evaluation down.
    //
    // Placing those shapes beside a spawn does not reach the panic, which is why
    // they are here and not in a fixture of their own. The natural optimisation
    // is a `line.contains("spawn")` pre-filter in front of the classifier, and a
    // hazardous line that does not say `spawn` never reaches it -- the byte
    // slicing ships intact behind the composition of the fixture. So the empty
    // line and the unprefixed Korean line sit *inside* the body of a spawn whose
    // span is attached below them, which a lookahead has to walk end to end to
    // reach: every line the window reads, it has to classify.
    let instrumented_over_hazards = " pub async fn drain() {\n     tokio::spawn(\n         async move {\n\n             // 개인정보 보호법 §24의2 감사 로그 적재\n개인정보 보호법 §24의2\n             let a = load().await;\n             let b = transform(a).await;\n             publish(b).await;\n         }\n         .instrument(tracing::info_span!(\"drain\")),\n     );\n }";
    let report = run(&diff_of(&[("src/worker.rs", instrumented_over_hazards)]));

    assert!(
        report.detached_findings.is_empty(),
        "this boundary attaches its span below its body, and the body is where \
         the hazardous lines are. The gate reported {} finding(s): {:?}, so it \
         blocks a merge over a defect the author did not commit -- and a window \
         that stops before the `.instrument(` is a window that never read the \
         lines it would have had to classify. Summary was: {}",
        report.detached_findings.len(),
        report
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>(),
        report.summary
    );
    assert_eq!(
        report.tasks_scanned, 1,
        "one async boundary is retained in this hunk. Summary was: {}",
        report.summary
    );

    // The raw-source shape: no diff prefix at all, a multi-byte first character,
    // and a spawn call on the line, so no `contains("spawn")` pre-filter can
    // route around the classifier. `SpanTracker::scan_detached_tasks` is already
    // called with unprefixed source in its own unit tests, so this is input the
    // gate really sees. The call carries its own span, so what is pinned is the
    // panic and not whether an unprefixed line counts as a line the pull request
    // ships -- that question is left where the rest of the prefix rules are.
    let unprefixed = run(&diff_of(&[(
        "src/worker.rs",
        "§ 감사: tokio::spawn(async move { audit().await; }.instrument(tracing::info_span!(\"audit\")));",
    )]));
    assert!(
        unprefixed.detached_findings.is_empty(),
        "this call attaches its span on the same line it opens on, so there is \
         no detached boundary here under any reading of the missing prefix; the \
         gate reported {:?}",
        unprefixed
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn added_and_retained_lines_are_counted_but_removed_ones_are_not() {
    // One addition, one unchanged context line, one removal, and one further
    // addition that attaches no span. Three boundaries exist in the merged
    // file, not four, and exactly one of them is detached. The detached one is
    // last, which is a fact about this fixture rather than a strengthening of
    // it: what constrains the *scope* of the lookahead, as against its width, is
    // the reversed-order case in
    // `a_file_that_instruments_one_spawn_and_forgets_the_other_...`, where a
    // detached spawn sits above an instrumented one. Here the ordering only
    // keeps this test about which lines were counted.
    let body = "+    tokio::spawn(async move { added().await; }.instrument(span()));\n     tokio::spawn(async move { kept().await; }.instrument(span()));\n-    tokio::spawn(async move { removed().await; }.instrument(span()));\n+    tokio::spawn(async move { no_span().await; });";
    let report = run(&diff_of(&[("src/worker.rs", body)]));

    assert_eq!(
        report.tasks_scanned, 3,
        "the merged file contains three spawns -- two added, one retained. The \
         fourth is being deleted and is not part of what this pull request \
         ships. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings.len(),
        1,
        "only the last spawn attaches no span; the removed line takes its own \
         defect with it and is not the author's to answer for. Findings were \
         {:?}. Summary was: {}",
        report
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>(),
        report.summary
    );

    // The identical hunk out of a CRLF-authored file, compared against the LF
    // run rather than against literals. What this reaches is narrower than it
    // looks, and it is stated rather than claimed away: `str::lines()` strips a
    // trailing `\r` before any matcher sees it, and a snippet built with
    // `.trim()` -- which the shipping code already does -- strips it again. So
    // against any `.lines()`-based *or* trimming scanner everything below holds
    // unconditionally, whether or not its author ever thought about line
    // endings. What it does bite is one plausible pair: a scanner that splits
    // the diff on `'\n'` itself *and* publishes the raw line, where an
    // end-anchored matcher or a `line.ends_with(");")` reclassifies the hunk and
    // the quoted text stops matching. Comparing the whole located set -- line
    // number and snippet together -- rather than only its size is what makes
    // that visible, and it is one comparison rather than three assertions.
    let crlf = run(&diff_of(&[("src/worker.rs", &as_crlf(body))]));
    let located = |r: &TraceContextReport| {
        let mut v: Vec<(usize, String)> = r
            .detached_findings
            .iter()
            .map(|f| (f.line_number, f.snippet.clone()))
            .collect();
        v.sort();
        v
    };

    assert_eq!(
        crlf.tasks_scanned, report.tasks_scanned,
        "the line endings of the file an author happens to work in are not a \
         property of their code; the CRLF hunk counted {} boundaries against {} \
         for the identical LF hunk. Summary was: {}",
        crlf.tasks_scanned, report.tasks_scanned, crlf.summary
    );
    assert_eq!(
        located(&crlf),
        located(&report),
        "the same one spawn is detached under either line ending, at the same \
         line, quoted the same way. A located set that moves with the line \
         endings is evidence the classifier was reading a character the author \
         did not write -- and a carriage return published into a pull request \
         comment renders as a broken line or a stray box."
    );

    // The lookahead reads lines too, and it has to classify them by the same
    // rule as the counter. The single most on-topic defect this gate could have
    // is a pull request that strips `.instrument(...)` off a spawn it keeps: the
    // spawn survives as a context line, the span leaves on a `-` line, and a
    // window that walks the raw hunk credits the retained boundary with a span
    // that is on its way out. The inverse is pinned beside it so that the fix is
    // "classify every line the window reads" and not "drop removed lines
    // everywhere", which would report a spawn as detached in the very pull
    // request that instruments it.
    let span_stripped = run(&diff_of(&[(
        "src/worker.rs",
        " pub async fn dispatch() {\n     tokio::spawn(async move {\n         work().await;\n-    }.instrument(tracing::info_span!(\"worker\")));\n+    });\n }",
    )]));

    assert_eq!(
        span_stripped.detached_findings.len(),
        1,
        "this pull request deletes the span from a spawn it keeps, which is the \
         defect this gate exists for. The gate reported {} finding(s): {:?}. \
         Zero means the lookahead read the `.instrument(` on the line being \
         removed and published a clean verdict over a boundary that ships \
         detached. Summary was: {}",
        span_stripped.detached_findings.len(),
        span_stripped
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>(),
        span_stripped.summary
    );
    assert!(
        span_stripped.detached_findings[0]
            .snippet
            .contains("spawn("),
        "the finding must quote the retained spawn, not the line being deleted; \
         got {:?}",
        span_stripped.detached_findings[0].snippet
    );
    assert!(
        !span_stripped.is_propagated,
        "the boundary this pull request keeps attaches no span once the hunk is \
         applied. Summary was: {}",
        span_stripped.summary
    );

    let span_attached = run(&diff_of(&[(
        "src/worker.rs",
        " pub async fn dispatch() {\n     tokio::spawn(async move {\n         work().await;\n-    });\n+    }.instrument(tracing::info_span!(\"worker\")));\n }",
    )]));

    assert!(
        span_attached.detached_findings.is_empty(),
        "this is the pull request that attaches the span, so the window has to \
         read the added line as part of what ships. The gate reported {:?}, \
         which fails the author for making the fix the gate asked for. Summary \
         was: {}",
        span_attached
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>(),
        span_attached.summary
    );
}
