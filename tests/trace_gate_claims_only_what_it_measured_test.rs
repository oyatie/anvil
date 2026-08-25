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
//! blocks a merge over a defect the author did not commit. Truncating the line
//! at its first `//` passes that row and opens a hole on the other side, so the
//! hole is pinned shut beside it: two further rows put a `://` inside a string
//! literal ahead of the call -- a shape this repository carries on 36 lines --
//! one where the truncation swallows a real detached boundary and publishes it
//! as nothing-in-scope, one where it swallows the `.instrument(...)` and the
//! gate accuses a correctly instrumented spawn. Passing all three fixes the
//! order: blank string literals first, strip line comments second.
//!
//! The receiver-method form is generated rather than spelled. An earlier draft
//! of this file wrote two `JoinSet::spawn` rows with different receiver names
//! and claimed that no allowlist of the exact spellings appearing here could
//! cover both. That was false: a six-entry allowlist covers them and greens the
//! file, while still missing every other receiver. So the form is now run as one
//! fixture body over a set of receiver identifiers the implementation has never
//! seen -- `js`, `handles`, `pool`, `tasks`, `workers` -- in a detached shape, an
//! instrumented shape and a `spawn_blocking` shape, and the gate's verdict is
//! required to be *identical* across all of them as well as correct in each. A
//! list of literals cannot green a set of names the fixture invents; only
//! recognising `<receiver>.spawn[_blocking](` as a form can. This repository has
//! no live `JoinSet` call site to cut the fixture from (`grep -rn JoinSet src/`
//! is empty), which is why this one shape is generated rather than lifted.
//! Recognised as a literal, the rows it misses are not flagged and not counted
//! either: uninstrumented boundaries published as "nothing was inspected",
//! which is section 1's defect wearing the other face.
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
//! around the classifier -- a body line the diff carries with no `+`, `-` or
//! leading space in front of it, which the chunk parser hands straight through.
//! That line attaches its own span, so it pins the panic without pinning
//! whether an unprefixed line counts as a line the pull request ships. `run()`
//! unwraps, so a panic is a hard red.
//!
//! ## Out of scope -- stated, rather than left ambiguous
//!
//! - The `.rs` chunk filter is pinned in the direction that bites this
//!   repository. An earlier draft of this file claimed that no fixture here
//!   could separate "the path was consulted" from "the chunk text was
//!   searched"; that claim was wrong and is withdrawn. The documentation-only
//!   row is a Markdown path whose body both shows an uninstrumented spawn in a
//!   fenced example *and* names `src/trace_context_guard/mod.rs` in its prose,
//!   so the chunk text contains `.rs` while the file being changed does not.
//!   `tasks_scanned == 0` with an empty finding set holds for a filter that
//!   consults the parsed `+++ b/<path>` and fails for the
//!   `file_diff.contains(".rs")` that ships today -- which four Markdown files
//!   in this repository, the ADR corpus among them, would already trip.
//!   The opposite direction is not pinned and cannot be with this helper:
//!   `diff_of` always emits `+++ b/{path}`, so a Rust file's chunk text always
//!   carries `.rs`, and no fixture here shows a Rust file skipped for want of
//!   the substring. That was not in this lane's verified scope.
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
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport};
use anvil::trace_context_guard::{TraceContextGuard, TraceContextReport};
use anvil::webhook::pipelines::review::scorecard_comment;
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
            // Two things are packed into this body, and both are load-bearing.
            //
            // It carries a fenced Rust example with an uninstrumented spawn in
            // it, so a scanner that reads every chunk regardless of the path it
            // belongs to finds the spawn, produces a finding, and publishes a
            // permanent merge block on any documentation change that shows an
            // uninstrumented spawn -- the accusation the loop's own assertion
            // forbids. Without it this row would be the spawn-free Rust row
            // below it written twice.
            //
            // And its prose names a `.rs` path, which is what separates a
            // filter that consults the path being changed from one that
            // searches the chunk text. What ships today is the second --
            // `if !file_diff.contains(".rs") { continue; }` -- and four
            // Markdown files in this repository already mention a `.rs` path,
            // the ADR corpus among them. So this chunk is Markdown by path and
            // contains `.rs` by text, and `tasks_scanned == 0` with an empty
            // finding set holds only for a filter that looks at the former.
            "a documentation-only pull request",
            &[(
                "docs/adr/0002-honesty.md",
                "+Gate 17 lives in `src/trace_context_guard/mod.rs`. The published name\n+must match the live measurement. For example, this is not a boundary in\n+this pull request, it is prose about one:\n+\n+```rust\n+tokio::spawn(async move { work().await; });\n+```",
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
            //
            // Its receiver is spelled `set`, which is deliberately not one of
            // the five names the generated receiver rows below use: an
            // allowlist assembled from those five still misses this one.
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
            // The pair below, read together with the comment row above it,
            // fixes the *order* of the two textual exclusions rather than
            // leaving it free. Truncating the line at its first `//` -- the
            // shortest way to pass the comment row -- silently discards
            // everything after a URL in a string literal, and this repository
            // carries 36 lines with a `://` in them. Here that discard drops a
            // real detached boundary: not flagged and not counted, published as
            // "nothing was inspected", which is section 1's defect wearing the
            // other face. The literal is kept on the *same* line as the call,
            // because that is the only position in which it reaches the bug.
            "a detached spawn behind a string literal containing //",
            "pub async fn dispatch() {\n    let url = \"https://api.example.com/hook\"; tokio::spawn(async move { post(url).await; });\n}",
            Detached,
            1,
        ),
        (
            // The same hazard pointed the other way: the `//` sits inside the
            // span's own name, so truncating at it throws away the
            // `.instrument(...)` that follows and the gate accuses a boundary
            // that is correctly instrumented. Passing this row and the comment
            // row together requires blanking string literals first and
            // stripping line comments second.
            "a span attached after a string literal containing //",
            "pub async fn dispatch() {\n    tokio::spawn(async move { post(\"https://api.example.com/hook\").await; }.instrument(tracing::info_span!(\"post https://api.example.com/hook\")));\n}",
            InstrumentedBoundary,
            1,
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
    let mut unseen: Vec<String> = Vec::new();
    let mut misaccused: Vec<String> = Vec::new();
    // Every row is exercised before anything is asserted, so one wrong row does
    // not hide the rest: the failure names all of them at once.
    let mut misreported: Vec<String> = Vec::new();

    for (label, code, expect, boundaries) in cases {
        let report = run(&diff_of(&[(FIXTURE_PATH, &as_added(code))]));
        let found = !report.detached_findings.is_empty();

        match (*expect, found) {
            (Detached, false) => unseen.push(label.to_string()),
            (InstrumentedBoundary, true) | (NotABoundary, true) => {
                misaccused.push(label.to_string())
            }
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

    // ---- a receiver-method spawn is a form, not a spelling ---------------
    //
    // `RECV` is substituted with each identifier below, so the same body is put
    // to the gate under five names it has never been shown. An allowlist of the
    // spellings that literally appear in this file -- which is what an earlier
    // draft of this row could be greened by -- cannot cover a set the fixture
    // invents; recognising `<receiver>.spawn[_blocking](` as a form is the only
    // thing that can.
    //
    // Two obligations are checked per shape, and the second is the one a pair of
    // hand-written rows cannot state. Each fixture must be classified correctly
    // in its own right *and* every receiver must be classified the same, so a
    // scanner that happens to know `workers` and not `pool` fails naming the
    // divergence rather than looking like a single wrong row.
    const RECEIVERS: &[&str] = &["js", "handles", "pool", "tasks", "workers"];
    let receiver_shapes: &[(&str, &str, Expect)] = &[
        (
            "RECV.spawn(..) with no span",
            "pub async fn dispatch() {\n    let mut RECV = tokio::task::JoinSet::new();\n    RECV.spawn(async move {\n        work().await;\n    });\n}",
            Detached,
        ),
        (
            "RECV.spawn(..) carrying a span",
            "pub async fn dispatch() {\n    let mut RECV = tokio::task::JoinSet::new();\n    RECV.spawn(async move {\n        work().await;\n    }.instrument(tracing::info_span!(\"worker\")));\n}",
            InstrumentedBoundary,
        ),
        (
            "RECV.spawn_blocking(..) with no span",
            "pub async fn dispatch() {\n    let mut RECV = tokio::task::JoinSet::new();\n    RECV.spawn_blocking(move || {\n        heavy_work();\n    });\n}",
            Detached,
        ),
    ];

    for (shape, code, expect) in receiver_shapes {
        // (receiver, tasks_scanned, findings) for each spelling of the same body.
        let mut observed: Vec<(&str, usize, usize)> = Vec::new();

        for receiver in RECEIVERS {
            let body = code.replace("RECV", receiver);
            let report = run(&diff_of(&[(FIXTURE_PATH, &as_added(&body))]));
            let row = shape.replace("RECV", receiver);
            let found = !report.detached_findings.is_empty();

            match (*expect, found) {
                (Detached, false) => unseen.push(row.clone()),
                (InstrumentedBoundary, true) => misaccused.push(row.clone()),
                _ => {}
            }
            if report.tasks_scanned != 1 {
                misreported.push(format!(
                    "{row}: this fixture contains one async boundary and the \
                     gate reported inspecting {}. Summary was: {}",
                    report.tasks_scanned, report.summary
                ));
            }
            observed.push((
                receiver,
                report.tasks_scanned,
                report.detached_findings.len(),
            ));
        }

        let first = (observed[0].1, observed[0].2);
        if observed
            .iter()
            .any(|(_, scanned, found)| (*scanned, *found) != first)
        {
            misreported.push(format!(
                "{shape}: renaming the receiver changed the gate's verdict over \
                 an identical body, so this form is being recognised as a list \
                 of spellings rather than as a shape. (receiver, scanned, \
                 findings) was {observed:?}"
            ));
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
    let cut = lines[start..end].join("\n");
    // The half of the rot guard the count above does not cover. Other lanes are
    // working this repository; one that attaches a span to these two threads
    // makes the cut window correct code, and every assertion below then fails
    // reading like a gate defect -- "the gate reported 0 finding(s)" -- which
    // invites the next agent to weaken a test that was right. If that happens
    // this fails first, and says what actually changed.
    assert!(
        !cut.contains(".instrument("),
        "fixture drawn from live source has rotted: the thread spawns in \
         {LIVE_FILE} now carry a span, so this hunk is no longer an example of \
         a dropped trace context. Re-cut it from a file that still has one, or \
         drop the row."
    );
    let hunk = as_added(&cut);

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
    // route around the classifier. A body line with no `+`, `-` or leading
    // space is handed through unchanged by the chunk parser, so this is input
    // the gate really sees. The call carries its own span, so what is pinned is the
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

// -------------------------------------------------------------------------
// 4. A boundary owns the region its own parenthesis closes -- no more, no less
// -------------------------------------------------------------------------

#[test]
fn the_outer_task_of_a_nested_spawn_is_cleared_by_the_span_attached_at_its_own_close() {
    // Cut from live source, because the shape is live here and the rule that
    // breaks it looks harmless in the abstract. `src/cli/server.rs` opens a
    // `tokio::spawn` whose body opens a second one, and the outer block closes
    // dozens of lines *below* the inner call. A rule that ends an outer
    // boundary's region where the next boundary begins can therefore never
    // reach a span attached at the outer close: the author who instruments that
    // task exactly as the finding tells them to is still accused, and no edit to
    // their file makes the gate pass. That is a fabricated accusation -- the
    // symmetric half of I1, named as such at `src/pre_merge_guard/report.rs` --
    // and it is not a smaller defect than the false clear it removes.
    //
    // Nothing here pins how the region is bounded. What is pinned is which
    // boundary the span at the outer close belongs to: the outer one, whose
    // parenthesis it is written inside, and not the inner one, which keeps its
    // own verdict.
    const LIVE_FILE: &str = "src/cli/server.rs";
    let source = std::fs::read_to_string(LIVE_FILE)
        .unwrap_or_else(|e| panic!("{LIVE_FILE} must be readable to build this fixture: {e}"));
    let lines: Vec<&str> = source.lines().collect();

    let opens = lines
        .iter()
        .position(|l| l.trim() == "tokio::spawn(async move {")
        .unwrap_or_else(|| {
            panic!(
                "fixture drawn from live source has rotted: {LIVE_FILE} no longer \
                 opens a spawn on a line of its own. Re-cut this fixture from a \
                 file that still nests one spawn inside another, or drop it."
            )
        });
    // The block ends at the last non-empty line before the next top-level comment.
    let next_section = lines
        .iter()
        .skip(opens)
        .position(|l| l.contains("Spawn periodic issue refresh"))
        .map(|k| opens + k)
        .unwrap_or_else(|| {
            panic!(
                "fixture drawn from live source has rotted: the comment that \
                 followed the outage-recovery block in {LIVE_FILE} is gone, so \
                 this cut can no longer find the end of the block. Re-cut it."
            )
        });
    let closes = (opens..next_section)
        .rev()
        .find(|&i| !lines[i].trim().is_empty())
        .expect("the block has at least one non-empty line");

    let cut: Vec<String> = lines[opens..=closes]
        .iter()
        .map(|l| l.to_string())
        .collect();
    assert_eq!(
        cut.iter()
            .filter(|l| l.trim() == "tokio::spawn(async move {")
            .count(),
        2,
        "fixture drawn from live source has rotted: the block cut from \
         {LIVE_FILE} no longer contains one spawn nested inside another, which \
         is the whole of what this test is about. Re-cut it or drop it."
    );
    assert!(
        !cut.iter().any(|l| l.contains(".instrument(")),
        "fixture drawn from live source has rotted: the block cut from \
         {LIVE_FILE} now attaches a span of its own, so this test can no longer \
         control which spans are present. Re-cut it or drop it."
    );
    assert_eq!(
        cut[cut.len() - 1].trim(),
        "});",
        "fixture drawn from live source has rotted: the block cut from \
         {LIVE_FILE} does not end at the parenthesis that closes the outer \
         spawn, so replacing that line no longer instruments the outer task."
    );

    // Calibration run: neither task carries a span, so both are detached and the
    // gate reports where each one is. The inner spawn's reported position is
    // read off this run rather than written down, so nothing here depends on how
    // the gate numbers its lines or on where in `server.rs` the block sits.
    let bare = run(&diff_of(&[(LIVE_FILE, &as_added(&cut.join("\n")))]));
    assert_eq!(
        bare.detached_findings.len(),
        2,
        "neither spawn in this block attaches a span, so both are detached. The \
         gate reported {} at {:?}. Summary was: {}",
        bare.detached_findings.len(),
        bare.detached_findings
            .iter()
            .map(|f| f.line_number)
            .collect::<Vec<_>>(),
        bare.summary
    );
    let mut bare_lines: Vec<usize> = bare
        .detached_findings
        .iter()
        .map(|f| f.line_number)
        .collect();
    bare_lines.sort_unstable();
    let (outer_line, inner_line) = (bare_lines[0], bare_lines[1]);

    // The same block with a span attached to the outer task, at the only place
    // the combinator can go: the line that closes the outer call.
    let mut instrumented = cut.clone();
    let last = instrumented.len() - 1;
    instrumented[last] = "    }.instrument(tracing::info_span!(\"outage_recovery\")));".to_string();
    let report = run(&diff_of(&[(
        LIVE_FILE,
        &as_added(&instrumented.join("\n")),
    )]));

    assert_eq!(
        report.tasks_scanned, 2,
        "both spawns are still boundaries the gate looked at. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings.len(),
        1,
        "the outer task now carries a span attached inside its own parentheses, \
         so only the inner one is detached. The gate reported {} finding(s) at \
         {:?} (outer opens at {outer_line}, inner at {inner_line}). Two means \
         the correctly instrumented outer task was accused with no edit \
         available that would clear it; zero means the outer task's span was \
         also credited to the inner one. Summary was: {}",
        report.detached_findings.len(),
        report
            .detached_findings
            .iter()
            .map(|f| f.line_number)
            .collect::<Vec<_>>(),
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].line_number, inner_line,
        "the surviving finding must be the inner spawn, which attaches nothing. \
         It was reported at line {} and the inner spawn is at {inner_line}. \
         Summary was: {}",
        report.detached_findings[0].line_number, report.summary
    );
}

#[test]
fn the_sibling_propagation_form_is_recognised_and_a_thread_is_told_a_fix_that_compiles() {
    // `Instrument::in_current_span()` is the other half of the same trait and
    // the canonical way to carry the caller's span across a spawn. Neither form
    // appears in this repository today, which is exactly the hazard: a gate that
    // knows only `.instrument(` fails the first pull request that reaches for
    // the idiomatic one, over a boundary that propagates context correctly.
    let carried = run(&diff_of(&[(
        "src/dispatch.rs",
        &as_added(
            "pub async fn dispatch() {\n    tokio::spawn(\n        async move {\n            work().await;\n        }\n        .in_current_span(),\n    );\n}",
        ),
    )]));
    assert!(
        carried.detached_findings.is_empty(),
        "this task carries the caller's span across the boundary, which is what \
         the gate exists to require. It was reported detached: {:?}. Summary \
         was: {}",
        carried
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>(),
        carried.summary
    );
    assert_eq!(
        carried.tasks_scanned, 1,
        "one boundary is in this diff and it was inspected. Summary was: {}",
        carried.summary
    );

    // A `std::thread::spawn` is a boundary the table above already pins as
    // detected. What it does not pin is what the author is then told to do, and
    // the obvious sentence prescribes a fix that does not build: `Instrumented<T>`
    // implements `Future` only when `T` does, and a thread is handed an
    // `FnOnce()`. So `closure.instrument(span)` fails to compile, and a gate
    // that blocks a merge on advice the compiler rejects is worse than one that
    // says nothing.
    let threaded = run(&diff_of(&[(
        "src/dispatch.rs",
        &as_added(
            "pub fn dispatch() {\n    let h = std::thread::spawn(move || {\n        pump();\n    });\n    let _ = h.join();\n}",
        ),
    )]));
    assert_eq!(
        threaded.detached_findings.len(),
        1,
        "the thread spawn is a boundary that carries no span. Summary was: {}",
        threaded.summary
    );
    let advice = threaded.detached_findings[0].issue.clone();
    assert!(
        !advice.contains(".instrument("),
        "a thread is given a closure, not a future, so `.instrument(..)` on it \
         does not compile under tracing 0.1. The gate told the author to write \
         it anyway: {advice:?}"
    );
    assert!(
        advice.to_lowercase().contains("enter"),
        "having ruled out the combinator, the finding has to name the fix that \
         does work -- entering the caller's span inside the closure -- or it \
         blocks a merge and leaves the author to guess. It said: {advice:?}"
    );
    assert!(
        !advice.to_lowercase().contains("asynchronous task"),
        "a `std::thread::spawn` is not an asynchronous task, and calling it one \
         sends the author looking for an async fix that does not exist here. \
         The finding said: {advice:?}"
    );

    // `tokio::task::spawn_blocking` is handed an `FnOnce() -> R` for the same
    // reason, and gate 4 (`rust_language_policy/engine.rs`) actively instructs
    // authors to move blocking calls onto it. A remedy decided by the substring
    // `thread::spawn` alone hands every other closure-taking boundary the
    // combinator that does not compile -- so gate 4 prescribes the form gate 17
    // then blocks with a fix the compiler rejects.
    let blocking = run(&diff_of(&[(
        "src/dispatch.rs",
        &as_added(
            "pub async fn dispatch() {\n    tokio::task::spawn_blocking(move || {\n        heavy();\n    });\n}",
        ),
    )]));
    assert_eq!(
        blocking.detached_findings.len(),
        1,
        "the blocking task is a boundary that carries no span. Summary was: {}",
        blocking.summary
    );
    let advice = blocking.detached_findings[0].issue.clone();
    assert!(
        !advice.contains(".instrument("),
        "`spawn_blocking` takes an `FnOnce() -> R` and `Instrumented<F>` is not \
         one, so the combinator does not compile here either. The gate told the \
         author to write it anyway: {advice:?}"
    );
    assert!(
        advice.to_lowercase().contains("enter"),
        "having ruled out the combinator, the finding has to name the fix that \
         does work -- entering the caller's span inside the closure. It said: \
         {advice:?}"
    );
    assert!(
        !advice.to_lowercase().contains("asynchronous task"),
        "the work handed to `spawn_blocking` is a closure, not an asynchronous \
         task; calling it one sends the author to an async fix that does not \
         apply. The finding said: {advice:?}"
    );
}

#[test]
fn a_boundary_whose_region_never_closes_in_the_lines_read_is_neither_cleared_nor_accused() {
    // git hands the gate three lines of context, so a hunk can carry a spawn and
    // stop before the parenthesis that closes it -- and `shipped_lines` flattens
    // every hunk of a file into one list, so a walk runs straight across the gap
    // between them and into unrelated code further down the file. The same thing
    // happens on any line whose parentheses the scanner miscounts.
    //
    // Neither verdict is available over a region whose extent was never
    // established. Publishing "a span is attached" credits this boundary with an
    // `.instrument(...)` written somewhere else entirely; publishing a finding
    // accuses it on evidence just as absent. So it is not counted among the
    // boundaries the summary reports having inspected, and it is not accused.
    let body = "+    tokio::spawn(async move {\n\
                +        let a = load().await;\n\
                +        publish(a).await;\n\
                @@ -80,3 +82,4 @@ pub async fn later() {\n\
                +    let guarded = other_future().instrument(tracing::info_span!(\"unrelated\"));\n\
                +    guarded.await;";

    let report = run(&diff_of(&[("src/worker.rs", body)]));

    assert_eq!(
        report.tasks_scanned, 0,
        "this hunk stops inside the spawn's body, so the region the boundary \
         owns was never established and no boundary here was inspected. \
         Counting it puts it under whatever verdict the summary publishes, on \
         the strength of an `.instrument(...)` that belongs to a different hunk \
         further down the file. Summary was: {}",
        report.summary
    );
    assert!(
        report.detached_findings.is_empty(),
        "an unestablished region is not evidence of a defect either; the gate \
         reported {} finding(s): {:?}. Summary was: {}",
        report.detached_findings.len(),
        report
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>(),
        report.summary
    );
    assert!(
        verification_claims_in(&report.summary).is_empty(),
        "nothing here was measured, so the gate holds no evidence to claim; it \
         published {:?} in: {}",
        verification_claims_in(&report.summary),
        report.summary
    );
    assert!(
        discloses_that_nothing_was_inspected(&report.summary),
        "the reader must be told that no boundary was classified rather than \
         left to read a clean verdict as a measurement; summary was: {}",
        report.summary
    );
    assert!(
        accusations_in(&report.summary).is_empty(),
        "the pull request is accused of nothing here; the gate published {:?} \
         in: {}",
        accusations_in(&report.summary),
        report.summary
    );
    assert!(
        report.is_propagated || !report.detached_findings.is_empty(),
        "the gate produced no finding against this diff and still reported it \
         as not propagating trace context, which `evaluator.rs:292` publishes \
         as a failed gate. Summary was: {}",
        report.summary
    );
}

// -------------------------------------------------------------------------
// 4. Review round 3: the region a verdict rests on has to be one that exists
//
// Everything above pins what the gate says about a region it established.
// These pin the establishing. A region assembled out of two unrelated hunks,
// or bounded by a parenthesis the scanner mis-read out of a comment, is not an
// extent that was measured -- and a verdict published over one is the same
// unbacked claim as the `verified` this lane removed, one layer down.
// -------------------------------------------------------------------------

/// Ways a summary asserts that it saw a span call inside a region. Kept apart
/// from `VERIFICATION_CLAIMS` because this is the *positive* half of the new
/// PASSED sentence: it does not say "verified", it says the call appears, which
/// is a statement about evidence just the same.
const APPEARANCE_CLAIMS: &[&str] = &["appears", "appear ", "attaches", "carries a span"];

fn appearance_claims_in(summary: &str) -> Vec<&'static str> {
    let lowered = summary.to_lowercase();
    APPEARANCE_CLAIMS
        .iter()
        .copied()
        .filter(|needle| lowered.contains(needle))
        .collect()
}

#[test]
fn a_region_is_neither_closed_nor_cleared_by_a_parenthesis_in_a_different_hunk() {
    // The hazard the test above this one names in its own comment, made to
    // fire. `shipped_lines` handed the scanner one flat list per file, so a
    // walk that opened in the first hunk ran straight over the `@@` header and
    // on into the second -- code that can sit hundreds of lines away in the
    // file.
    //
    // The second hunk here supplies exactly the two things such a walk needs to
    // manufacture a verdict: a stray `)` that balances the boundary's
    // parenthesis, and an `.instrument(...)` belonging to a different call
    // entirely. Neither is evidence about the spawn in the first hunk. The
    // region that spawn opens is established nowhere in what the gate was
    // handed, so it is unresolved -- not counted, not accused, and above all
    // not cleared.
    let body = "+    tokio::spawn(async move {\n\
                +        let a = load().await;\n\
                +        publish(a).await;\n\
                @@ -300,4 +302,5 @@ pub async fn later() {\n\
                +    let g = fut().instrument(tracing::info_span!(\"unrelated\"));\n\
                +    });";

    let report = run(&diff_of(&[("src/worker.rs", body)]));

    assert_eq!(
        report.tasks_scanned, 0,
        "the boundary opens in the first hunk and nothing in that hunk closes \
         it. Counting it as inspected puts it under whatever verdict the \
         summary publishes, on the strength of a parenthesis and a span call \
         from a hunk that is somewhere else in the file. Summary was: {}",
        report.summary
    );
    assert!(
        report.detached_findings.is_empty(),
        "an unestablished region is not evidence of a defect either; the gate \
         reported {} finding(s). Summary was: {}",
        report.detached_findings.len(),
        report.summary
    );
    assert!(
        appearance_claims_in(&report.summary).is_empty(),
        "the gate may not tell the reader that a span call appears inside this \
         boundary's region: it never established where that region ends. It \
         published {:?} in: {}",
        appearance_claims_in(&report.summary),
        report.summary
    );
    assert!(
        numeric_tokens(&report.summary).contains(&1),
        "one boundary was seen and could not be classified, and the reader is \
         owed that number rather than a sentence reporting that nothing was \
         there at all. The numbers published were {:?}. Summary was: {}",
        numeric_tokens(&report.summary),
        report.summary
    );
    assert!(
        accusations_in(&report.summary).is_empty(),
        "the pull request is accused of nothing here; the gate published {:?} \
         in: {}",
        accusations_in(&report.summary),
        report.summary
    );
    assert!(
        report.is_propagated || !report.detached_findings.is_empty(),
        "the gate produced no finding against this diff and still reported it \
         as not propagating trace context. Summary was: {}",
        report.summary
    );
}

#[test]
fn every_boundary_on_a_line_is_inspected_not_merely_the_first() {
    // Two spawns on one line, the first instrumented and the second not. A
    // scanner that takes the first match per line never sees the second: not
    // classified, not accused, and -- because the published count is the number
    // it did classify -- the sentence reports one boundary in scope when two
    // were. That is a green check over a task that ships detached, which is
    // this lane's whole subject.
    let report = run(&diff_of(&[(
        "src/dispatch.rs",
        "+    tokio::spawn(traced().instrument(tracing::info_span!(\"traced\"))); tokio::spawn(bare());",
    )]));

    assert_eq!(
        report.tasks_scanned, 2,
        "two calls on this line open async boundaries, and the sentence \
         reports how many were inspected. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings.len(),
        1,
        "the first call carries a span and the second carries none, so exactly \
         one finding is correct. The gate reported {}: {:?}. Zero means the \
         second boundary was never looked at; two means the instrumented one \
         was accused. Summary was: {}",
        report.detached_findings.len(),
        report
            .detached_findings
            .iter()
            .map(|f| f.snippet.clone())
            .collect::<Vec<_>>(),
        report.summary
    );
    assert!(
        !report.is_propagated,
        "a boundary on this line drops the trace context. Summary was: {}",
        report.summary
    );
    assert!(
        numeric_tokens(&report.summary).contains(&2),
        "the count of boundaries inspected has to be the number of boundaries \
         inspected. The numbers published were {:?}. Summary was: {}",
        numeric_tokens(&report.summary),
        report.summary
    );
}

#[test]
fn a_published_location_is_the_post_image_line_the_hunk_header_declares() {
    // The FAILED sentence ends `at <path>:<n>`, which reads as a file line and
    // therefore has to be one. A number counted over the split diff chunk --
    // its `index`, `---`, `+++` and `@@` headers included, and never restarted
    // per hunk -- sends a reviewer to a line that has nothing to do with the
    // accusation.
    //
    // The hunk header is the only thing in a diff that says where a body sits,
    // so this fixture declares one that does not begin at line 1 and puts a
    // context line ahead of the spawn. `+302` plus one context line is 303, and
    // no counting over the chunk arrives there by accident.
    let body = "@@ -300,4 +302,6 @@ pub async fn later() {\n\
                     let prepared = prepare();\n\
                +    tokio::spawn(async move { work(prepared).await; });\n\
                     let _ = done();";

    let report = run(&diff_of(&[("src/worker.rs", body)]));

    assert_eq!(
        report.detached_findings.len(),
        1,
        "one boundary here attaches no span. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].line_number, 303,
        "the hunk header says its body begins at post-image line 302, and one \
         context line precedes the spawn, so the spawn is at line 303. The \
         gate reported {} -- a number counted over the diff chunk and \
         published to a reviewer as a source location. Summary was: {}",
        report.detached_findings[0].line_number, report.summary
    );
    assert!(
        report.summary.contains("src/worker.rs:303"),
        "the sentence a reviewer reads carries the location, so it has to \
         carry the right one. Summary was: {}",
        report.summary
    );
}

#[test]
fn a_span_call_that_is_only_prose_inside_a_literal_clears_nothing() {
    // The dominant multi-line string idiom in this repository is a
    // backslash-continued ordinary literal, and this pull request's own new
    // code uses it to write *about* `.instrument(...)`. A stripper that resets
    // its string state at every line boundary lexes those continuation lines as
    // live code, so the words inside the literal are read as a span call and
    // clear a task that ships detached -- the exact defect this gate exists to
    // find, reintroduced one layer down.
    let body = "+    tokio::spawn(async move {\n\
                +        tracing::warn!(\n\
                +            \"worker started without a span; attach one with \\\n\
                +             `.instrument(...)` before shipping\"\n\
                +        );\n\
                +        work().await;\n\
                +    });";

    let report = run(&diff_of(&[("src/worker.rs", body)]));

    assert_eq!(
        report.tasks_scanned, 1,
        "one boundary is in this hunk and it opens and closes inside it. \
         Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings.len(),
        1,
        "this task attaches no span: the only `.instrument(` in the hunk is \
         prose inside a string literal that runs across two lines. The gate \
         reported {} finding(s) and published: {}",
        report.detached_findings.len(),
        report.summary
    );
}

#[test]
fn a_parenthesis_the_scanner_cannot_vet_does_not_close_a_region_early() {
    // The mirror of the case above, and the one that blocks merges. A `)`
    // inside a char literal, a block comment or a raw string is not code, and a
    // scanner that counts it closes the region early -- fully classified, so
    // the author is told that no span call appears inside a region whose
    // closing line plainly carries one, and no edit short of deleting the
    // literal makes the gate pass.
    let cases: &[(&str, &str)] = &[
        (
            "a char literal holding a closing parenthesis",
            "pub async fn drain() {\n    tokio::spawn(async move {\n        let closer = ')';\n        work(closer).await;\n    }.instrument(tracing::info_span!(\"drain\")));\n}",
        ),
        (
            "a block comment holding a closing parenthesis",
            "pub async fn drain() {\n    tokio::spawn(async move {\n        /* returns ) on failure */\n        work().await;\n    }.instrument(tracing::info_span!(\"drain\")));\n}",
        ),
        (
            "a raw string holding a closing parenthesis",
            "pub async fn drain() {\n    tokio::spawn(async move {\n        let q = r#\"SELECT f(a) ) FROM t\"#;\n        run(q).await;\n    }.instrument(tracing::info_span!(\"drain\")));\n}",
        ),
    ];

    for (label, code) in cases {
        let report = run(&diff_of(&[("src/worker.rs", &as_added(code))]));
        assert_eq!(
            report.tasks_scanned, 1,
            "{label}: one boundary is in this hunk. Summary was: {}",
            report.summary
        );
        assert!(
            report.detached_findings.is_empty(),
            "{label}: the span is attached on the line that closes this call, \
             so this boundary is instrumented. The gate accused it anyway -- a \
             merge blocked over a defect that is not there, with no edit \
             available that clears it short of deleting the literal. Summary \
             was: {}",
            report.summary
        );
    }

    // The other direction of the same miscount, and the reason it is the lexer
    // that has to be fixed rather than the arithmetic: a block comment that
    // merely writes *about* a spawn is otherwise scanned as one.
    let commented = run(&diff_of(&[(
        "src/worker.rs",
        &as_added(
            "pub async fn drain() {\n    /*\n     * The old shape was tokio::spawn(async move { work().await; });\n     */\n    work().await;\n}",
        ),
    )]));
    assert_eq!(
        commented.tasks_scanned, 0,
        "a spawn written inside a block comment is not a boundary this pull \
         request ships. Summary was: {}",
        commented.summary
    );
    assert!(
        commented.detached_findings.is_empty(),
        "the gate reported {} finding(s) against prose. Summary was: {}",
        commented.detached_findings.len(),
        commented.summary
    );

    // And a spawn written inside a multi-line raw string -- the shape this
    // repository's own diff fixtures are built out of.
    let fixture_literal = run(&diff_of(&[(
        "src/worker.rs",
        &as_added("pub const HUNK: &str = r#\"\ntokio::spawn(async move { work().await; });\n\"#;"),
    )]));
    assert_eq!(
        fixture_literal.tasks_scanned, 0,
        "a spawn inside a raw string is a fixture, not a task this pull \
         request starts. Summary was: {}",
        fixture_literal.summary
    );
    assert!(
        fixture_literal.detached_findings.is_empty(),
        "the gate reported {} finding(s) against a string literal. Summary \
         was: {}",
        fixture_literal.detached_findings.len(),
        fixture_literal.summary
    );
}

#[test]
fn the_remedy_the_gate_computed_reaches_the_sentence_the_author_reads() {
    // `summary` is the only field of this report that any published surface
    // renders: `evaluator.rs:292-296` clones it and drops `detached_findings`.
    // So a remedy computed per finding and left in that vector is advice nobody
    // is ever shown, and the author instead reads the generic sentence -- which,
    // for a `std::thread::spawn`, prescribes a combinator that does not compile
    // on a closure.
    let threaded = run(&diff_of(&[(
        "src/dispatch.rs",
        &as_added(
            "pub fn dispatch() {\n    let h = std::thread::spawn(move || {\n        pump();\n    });\n    let _ = h.join();\n}",
        ),
    )]));

    assert_eq!(
        threaded.detached_findings.len(),
        1,
        "the thread spawn is a boundary that carries no span. Summary was: {}",
        threaded.summary
    );
    for finding in &threaded.detached_findings {
        assert!(
            threaded.summary.contains(&finding.issue),
            "the gate worked out what to tell the author about {}:{} and then \
             published a sentence that does not contain it. The remedy was \
             {:?}; the sentence was: {}",
            finding.file_path,
            finding.line_number,
            finding.issue,
            threaded.summary
        );
    }
    assert!(
        threaded.summary.to_lowercase().contains("enter"),
        "a thread is handed a closure, not a future, so the fix is to enter \
         the caller's span inside it. The sentence the author actually reads \
         said: {}",
        threaded.summary
    );
}

/// Ways a summary tells the reader that a line it names may not be one this
/// change wrote. Any one of them is enough; no phrasing is required.
const RETENTION_DISCLOSURES: &[&str] = &["retain", "kept", "pre-existing", "already"];

#[test]
fn an_accusation_says_the_line_it_names_may_be_one_the_change_only_kept() {
    // The gate reads added *and retained* lines, which is right -- a region
    // walk that skipped context lines could bound nothing. The cost is that an
    // author can be failed over a spawn they did not write and only carried
    // past in three lines of context. Twenty-nine such boundaries are live in
    // this repository today and not one of them carries a span, so this is the
    // ordinary case rather than the corner one, and the sentence has to say so
    // or it accuses the wrong person.
    let body = "     tokio::spawn(async move {\n\
                         legacy_work().await;\n\
                     });\n\
                +    let _ = touched();";

    let report = run(&diff_of(&[("src/legacy.rs", body)]));

    assert_eq!(
        report.detached_findings.len(),
        1,
        "the retained boundary carries no span, and the gate reads retained \
         lines. Summary was: {}",
        report.summary
    );
    let lowered = report.summary.to_lowercase();
    assert!(
        RETENTION_DISCLOSURES.iter().any(|n| lowered.contains(n)),
        "every line this sentence names may be one the pull request only kept \
         as a context line. Told nothing, the author reads it as a defect they \
         introduced and goes looking through their own change for it. Summary \
         was: {}",
        report.summary
    );
}

// -------------------------------------------------------------------------
// 5. The verdict the guard reached is the verdict the reader is shown
// -------------------------------------------------------------------------

/// The scorecard body Anvil actually posts, for a certification report in which
/// `trace_status` is whatever the guard decided and every other gate is left
/// unmeasured.
///
/// The baseline matters: `publish::scorecard::render` enumerates findings and
/// says nothing whatever about a gate it considers passing, so "the sentence
/// reached a reader" is not a property of the string the guard composed. It is
/// a property of the status the guard hands the evaluator, and it can only be
/// observed by rendering.
fn published_with_trace_status(status: &GateStatus) -> String {
    let mut report = PreMergeCertificationReport::unmeasured(
        "baseline for this fixture: only gate 17 is exercised here",
    );
    report.trace_status = status.clone();
    report.seal();
    scorecard_comment(&report)
}

/// The four diffs that reach the four arms of the guard's verdict, each with a
/// short name for what it is.
fn arm_fixtures() -> Vec<(&'static str, TraceContextReport)> {
    vec![
        (
            "nothing-to-measure",
            run(&diff_of(&[("src/plain.rs", "+pub const ROWS: usize = 3;")])),
        ),
        (
            "clean",
            run(&diff_of(&[(
                "src/clean.rs",
                &as_added(
                    "pub async fn go() {\n    tokio::spawn(work().instrument(tracing::info_span!(\"w\")));\n}",
                ),
            )])),
        ),
        (
            "detached",
            run(&diff_of(&[(
                "src/detached.rs",
                &as_added(
                    "pub async fn go() {\n    tokio::spawn(async move { work().await; });\n}",
                ),
            )])),
        ),
        (
            "unresolved",
            run(&diff_of(&[(
                "src/unresolved.rs",
                "+    tokio::spawn(async move {\n+        work().await;",
            )])),
        ),
    ]
}

#[test]
fn every_sentence_the_guard_composes_is_carried_by_the_status_it_hands_over() {
    // The guard writes four sentences and the pipeline published one of them.
    // `GateStatus::Passed` is a unit variant, so a status rebuilt downstream
    // from `is_propagated` discards the other three before any reader sees
    // them -- a gate formatting prose it knows is thrown away is the same
    // unmeasured assurance this lane exists to remove, one layer up.
    //
    // What is pinned here is that the guard *decides* and the decision travels:
    // whatever verdict it reached, the status it hands over carries the
    // sentence it wrote, so no mapping downstream can drop it.
    for (name, report) in arm_fixtures() {
        let carried = match &report.status {
            GateStatus::Passed | GateStatus::AutoUpdated => None,
            GateStatus::Warning(s) | GateStatus::Failed(s) | GateStatus::Errored(s) => Some(s),
            GateStatus::NotMeasured { reason, .. } => Some(reason),
            // The empty-scope arm. It was `Warning` while `NotMeasured` blocked
            // admission unconditionally; now that a subject set found empty has
            // its own variant, the guard says what happened instead of picking
            // the least-wrong status. The sentence still travels, which is what
            // this test pins.
            GateStatus::NotApplicable { subject, .. } => Some(subject),
        };
        if name == "clean" {
            // A measurement that ran and found every boundary instrumented has
            // nothing to report, and the scorecard's rule is findings only. It
            // is the one arm entitled to be silent.
            assert!(
                matches!(report.status, GateStatus::Passed),
                "a diff whose boundaries were inspected and carry spans is a \
                 pass. The guard published {:?} with summary: {}",
                report.status,
                report.summary
            );
        } else {
            assert_eq!(
                carried,
                Some(&report.summary),
                "the {name} arm composed a sentence and handed over a status \
                 that does not carry it. Status was {:?}; the sentence the \
                 guard wrote was: {}",
                report.status,
                report.summary
            );
        }
    }
}

#[test]
fn a_diff_the_gate_found_no_boundary_in_says_so_on_the_scorecard() {
    // The published claim issue #14 filed: a pull request that crosses no async
    // boundary renders as a bare tick inside `Certified -- N/N gates passed`,
    // one of which inspected nothing. The sentence that says nothing was
    // inspected has to reach the surface the reviewer reads, not merely the
    // struct field the evaluator drops.
    let report = run(&diff_of(&[("src/plain.rs", "+pub const ROWS: usize = 3;")]));
    let published = published_with_trace_status(&report.status);

    assert!(
        published.contains(&report.summary),
        "the guard wrote {:?} and the scorecard published none of it:\n{published}",
        report.summary
    );
    assert!(
        published.contains("**trace**"),
        "the row has to name the gate, or the reader cannot tell which of \
         seventy-two inspected nothing:\n{published}"
    );
}

#[test]
fn a_boundary_the_gate_saw_and_could_not_judge_is_disclosed_rather_than_passed() {
    // `unresolved` counts a boundary that was *there* and could not be
    // classified -- its parenthesis closes outside the hunk that opened it.
    // Routed to the same badge and the same status as "there was nothing to
    // look at", an uninstrumented spawn a pull request adds ships green and
    // undisclosed. The corpus keeps the two apart: `coverage_guard.rs` maps
    // nothing-to-look-at to `Passed` and a thing it could not measure to
    // `NotMeasured`.
    let unresolved = run(&diff_of(&[(
        "src/unresolved.rs",
        "+    tokio::spawn(async move {\n+        work().await;",
    )]));
    let empty = run(&diff_of(&[("src/plain.rs", "+pub const ROWS: usize = 3;")]));

    assert_ne!(
        unresolved.status, empty.status,
        "a boundary that was seen and could not be judged is not the same \
         finding as a diff with no boundary in it, and the two must not reach \
         the reader under one status. The sentences were {:?} and {:?}",
        unresolved.summary, empty.summary
    );
    assert!(
        !matches!(unresolved.status, GateStatus::Passed),
        "the gate saw a task boundary and reached no verdict about it; \
         publishing that as a pass is the false green this lane exists to \
         close. Summary was: {}",
        unresolved.summary
    );

    let published = published_with_trace_status(&unresolved.status);
    assert!(
        published.contains(&unresolved.summary),
        "the count of boundaries the gate could not judge, and the reason, \
         have to reach the surface the reviewer reads. The guard wrote {:?} \
         and the scorecard published:\n{published}",
        unresolved.summary
    );
    assert!(
        numeric_tokens(&unresolved.summary).contains(&1),
        "one boundary was seen and not judged, and the reader is owed that \
         number. The numbers published were {:?}. Summary was: {}",
        numeric_tokens(&unresolved.summary),
        unresolved.summary
    );
}

// -------------------------------------------------------------------------
// 6. A location the gate publishes is a line the file has
// -------------------------------------------------------------------------

#[test]
fn the_marker_git_writes_for_a_missing_final_newline_is_not_a_shipped_line() {
    // `\ No newline at end of file` is written by git into the middle of a hunk
    // whenever the pre-image's final line lacked a trailing newline and is
    // being replaced. It is not a body line: it occupies no position in either
    // image. Counted as one, every location after it in that hunk is published
    // one too high -- and on a two-line post-image, past the end of the file.
    //
    // The author does nothing unusual to produce it, so this defeats the
    // post-image guarantee on ordinary git output.
    let replaced = "@@ -1,2 +1,2 @@\n\
                    -old\n\
                    \\ No newline at end of file\n\
                    +new\n\
                    +tokio::spawn(work());";
    let report = run(&diff_of(&[("src/m.rs", replaced)]));
    assert_eq!(
        report.detached_findings.len(),
        1,
        "one boundary here attaches no span. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].line_number, 2,
        "the post-image of this hunk is two lines -- `new`, then the spawn -- \
         so the spawn is at line 2. A marker counted as a body line pushes \
         every later location past the end of the file. Summary was: {}",
        report.summary
    );

    let mid_hunk = "@@ -1,4 +1,5 @@\n\
                     fn a() {}\n\
                    \\ No newline at end of file\n\
                    +fn b() {}\n\
                     tokio::spawn(async move { work().await; });";
    let report = run(&diff_of(&[("src/w.rs", mid_hunk)]));
    assert_eq!(
        report.detached_findings.len(),
        1,
        "one boundary here attaches no span. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings[0].line_number, 3,
        "two lines ship above the spawn, so it is at post-image line 3; the \
         marker between them is not one of them. Summary was: {}",
        report.summary
    );
    assert!(
        report.summary.contains("src/w.rs:3"),
        "the sentence a reviewer reads carries the location, so it has to \
         carry the right one. Summary was: {}",
        report.summary
    );
}

// -------------------------------------------------------------------------
// 7. The gate reads every Rust chunk it was handed
// -------------------------------------------------------------------------

#[test]
fn a_rust_body_that_contains_the_words_of_a_diff_header_is_still_read() {
    // The chunk splitter cut the diff on the bare substring `diff --git`, which
    // occurs in body text as readily as in a header. Everything after it in
    // that file's diff lands in a fragment with no `+++ ` header, so the path
    // is unreadable and the rest of the file is dropped without a word -- and
    // the gate then publishes that it found no boundary.
    //
    // That is absent evidence rendered as a pass, the failure this lane exists
    // to close, and it is a one-line evasion available to any author on a
    // watched repository. A comment is enough.
    let commented = "+    // context: diff --git a/x b/x\n\
                     +    tokio::spawn(async move { evil().await; });";
    let report = run(&diff_of(&[("src/fleet_probe.rs", commented)]));
    assert_eq!(
        report.tasks_scanned, 1,
        "the hunk plainly contains one task boundary and the gate reported \
         that it read none. Summary was: {}",
        report.summary
    );
    assert_eq!(
        report.detached_findings.len(),
        1,
        "that boundary attaches no span. Summary was: {}",
        report.summary
    );

    // The same literal inside a string, which is how it reaches this
    // repository's own test suite: `diff_of` above writes it on every fixture,
    // so the gate was blind to the file pinning its behaviour.
    let quoted = "+    let fixture = \"diff --git a/src/q.rs b/src/q.rs\";\n\
                  +    tokio::spawn(async move { work().await; });";
    let report = run(&diff_of(&[("tests/f.rs", quoted)]));
    assert_eq!(
        report.detached_findings.len(),
        1,
        "a Rust body quoting a diff header is still Rust. The gate reported {} \
         finding(s) and published: {}",
        report.detached_findings.len(),
        report.summary
    );
}

// -------------------------------------------------------------------------
// 8. The sentence states the predicate the code evaluated
// -------------------------------------------------------------------------

#[test]
fn the_failing_sentence_states_the_predicate_the_gate_actually_evaluated() {
    // The scan diverts the text of a nested boundary into that boundary's own
    // frame, so what it tests is "inside the region it opens, *minus the
    // regions its nested boundaries own*". The published sentence dropped the
    // qualifier and asserted the stronger claim -- on the one surface an author
    // reads while looking straight at the `.instrument(` the sentence says is
    // not there.
    let nested = run(&diff_of(&[(
        "src/y.rs",
        "+tokio::spawn(async move { set.spawn(child().instrument(sp())); other().await; });",
    )]));

    assert_eq!(
        nested.detached_findings.len(),
        1,
        "the outer task carries no span of its own; the inner one does. \
         Summary was: {}",
        nested.summary
    );
    assert!(
        nested.summary.to_lowercase().contains("nest"),
        "the reader is looking at an `.instrument(` inside the parentheses the \
         accused line opens, and the sentence tells them there is none. It has \
         to name the exclusion the code applies -- the regions of the \
         boundaries nested inside this one. It said: {}",
        nested.summary
    );

    // A sentence that reports one finding among one boundary and calls it
    // "1 of 1 task boundaries" is a format string that never learned to count.
    let single = run(&diff_of(&[(
        "src/single.rs",
        "+tokio::spawn(async move { work().await; });",
    )]));
    assert!(
        !single.summary.contains("1 of 1 task boundaries"),
        "one boundary is a boundary. Summary was: {}",
        single.summary
    );
}

// -------------------------------------------------------------------------
// 9. Each boundary is told the fix for its own callee
// -------------------------------------------------------------------------

/// The two remedies a finding can carry, told apart by what they instruct.
fn is_closure_remedy(issue: &str) -> bool {
    !issue.contains(".instrument(") && issue.to_lowercase().contains("enter")
}

#[test]
fn each_boundary_on_a_line_carries_the_remedy_for_its_own_callee() {
    // The remedy was read off everything to the left of the call's parenthesis
    // on the line, so for the second and later boundaries the prefix included
    // the earlier calls and the kind was taken from the wrong callee. Both
    // findings then carry one string, `remedies()` dedupes them to it, and the
    // author of the async task is told to hold a span guard across an await --
    // the anti-pattern -- while the advice that fits it is never published at
    // all. A misattributed remedy that reaches the reader is worse than one
    // nobody saw.
    let orderings = [
        "+let a = std::thread::spawn(move || work()); let b = tokio::spawn(async move { work().await; });",
        "+let b = tokio::spawn(async move { work().await; }); let a = std::thread::spawn(move || work());",
    ];
    for body in orderings {
        let report = run(&diff_of(&[("src/mixed.rs", body)]));
        assert_eq!(
            report.detached_findings.len(),
            2,
            "both calls on this line open a boundary and neither carries a \
             span. Summary was: {}",
            report.summary
        );
        let issues: Vec<String> = report
            .detached_findings
            .iter()
            .map(|f| f.issue.clone())
            .collect();
        assert_eq!(
            issues.iter().filter(|i| is_closure_remedy(i)).count(),
            1,
            "exactly one of these boundaries is handed a closure. The gate \
             attributed: {issues:?}. Summary was: {}",
            report.summary
        );
        assert_eq!(
            issues.iter().filter(|i| i.contains(".instrument(")).count(),
            1,
            "exactly one of these boundaries is handed a future. The gate \
             attributed: {issues:?}. Summary was: {}",
            report.summary
        );
        for issue in &issues {
            assert!(
                report.summary.contains(issue),
                "the gate worked out {issue:?} and published a sentence that \
                 does not contain it: {}",
                report.summary
            );
        }
    }
}

// -------------------------------------------------------------------------
// 10. A definition is not a call
// -------------------------------------------------------------------------

#[test]
fn a_declaration_named_spawn_is_not_a_call_that_opens_a_task() {
    // The matcher is nominal, so `fn spawn(task: BoxFuture) {` reads as a call
    // whose argument list is not empty and is accused. That matters
    // operationally: the remedy for a fleet of detached boundaries is a wrapper
    // that attaches the span, the obvious name for it is `spawn`, and its
    // declaration line is the one place a span is actually attached.
    for declaration in [
        "+fn spawn(task: BoxFuture) {\n+    inner(task);\n+}",
        "+pub async fn spawn(task: BoxFuture) {\n+    inner(task);\n+}",
        "+trait Runtime {\n+    fn spawn(&self, f: BoxFuture);\n+}",
    ] {
        let report = run(&diff_of(&[("src/rt.rs", declaration)]));
        assert_eq!(
            report.tasks_scanned, 0,
            "a definition is not a call and opens no task. The gate inspected \
             {} in:\n{declaration}\nSummary was: {}",
            report.tasks_scanned, report.summary
        );
        assert!(
            report.detached_findings.is_empty(),
            "nothing here is accused. The gate published: {}",
            report.summary
        );
    }

    // And the exclusion is of the declaration, not of the file: a call inside
    // a function named `spawn` is still a call.
    let wrapper = run(&diff_of(&[(
        "src/rt.rs",
        "+pub fn spawn(fut: F) {\n+    tokio::spawn(fut);\n+}",
    )]));
    assert_eq!(
        wrapper.tasks_scanned, 1,
        "the body of the wrapper spawns a task and that call is a boundary. \
         Summary was: {}",
        wrapper.summary
    );
}

// -------------------------------------------------------------------------
// 11. What the gate accuses wrongly is disclosed, not only what it misses
// -------------------------------------------------------------------------

/// The `trace_status` gap text, which this repository treats as the honesty
/// surface for what a gate does not do.
fn trace_gap() -> &'static str {
    anvil::fidelity::registry::AUDITED_GATES
        .iter()
        .find(|g| g.gate_id == "trace_status")
        .expect("gate 17 must be audited in the fidelity registry")
        .gap
}

#[test]
fn the_span_hoisted_out_of_the_call_is_accused_and_the_registry_says_so() {
    // Building the future in steps and handing the binding to `spawn` is
    // idiomatic and is the shape any non-trivial spawn takes. The span is
    // attached outside the parenthesised region, so the region walk cannot see
    // it and the boundary is reported detached -- a `Failed`, which blocks the
    // merge. The registry disclosed only the direction that quietly passes.
    let report = run(&diff_of(&[(
        "src/hoisted.rs",
        "+let fut = work().instrument(tracing::info_span!(\"w\"));\n+tokio::spawn(fut);",
    )]));
    assert_eq!(
        report.detached_findings.len(),
        1,
        "this is the shape the gate gets wrong, and the fixture exists so the \
         disclosure and the behaviour cannot drift apart. Summary was: {}",
        report.summary
    );
    let gap = trace_gap().to_lowercase();
    assert!(
        gap.contains("attached before"),
        "a gap section that lists only the direction which quietly passes, and \
         omits the direction which blocks, understates the gate exactly where \
         it costs the fleet. The gap said: {}",
        trace_gap()
    );
}

#[test]
fn the_registry_says_the_matcher_is_nominal_and_has_no_allowlist() {
    // Any non-empty call whose final path segment is `spawn`, `spawn_blocking`
    // or `spawn_local` is treated as a task boundary: a thread pool, a process
    // supervisor, an actor handle, a domain type of one's own. It is accused
    // and it blocks, and there is no way to say otherwise.
    let report = run(&diff_of(&[(
        "src/p.rs",
        "+let h = pool.spawn(job_description);",
    )]));
    assert_eq!(
        report.detached_findings.len(),
        1,
        "the matcher cannot tell this from a runtime task; the fixture pins \
         the behaviour the gap has to describe. Summary was: {}",
        report.summary
    );
    let gap = trace_gap().to_lowercase();
    assert!(
        gap.contains("allowlist"),
        "the gap enumerates the false negatives and never says the gate will \
         fail a merge over a method that is not a task boundary at all. The \
         gap said: {}",
        trace_gap()
    );
}

// -------------------------------------------------------------------------
// 12. The region walk is linear in the text it was handed
// -------------------------------------------------------------------------

/// Deep enough that a per-character walk of the parenthesis stack is visibly
/// quadratic, small enough that a linear one is instant. One line, so the cost
/// measured is the region walk and not the diff parsing around it.
const DEEP_NEST: usize = 40_000;

/// Wall clock is a blunt instrument, so the margin is enormous rather than
/// tight: the walk this pins runs in tens of milliseconds and the ceiling is
/// three seconds. The shape it rejects took 16 s on this input in a debug
/// build, so nothing between "linear" and "quadratic" is being split here.
const LINEAR_CEILING: std::time::Duration = std::time::Duration::from_secs(3);

#[test]
fn the_region_walk_does_not_go_quadratic_on_webhook_supplied_text() {
    // `diff_content` is `git diff` output for a pull request on a watched
    // repository: attacker-controlled, and this gate applies no size cap where
    // `cedar_guard.rs` and `doc_guard/mod.rs` bound theirs at 50,000 characters.
    // Appending each character to the innermost open region by *searching* the
    // stack for it costs O(depth) per character, so a single deeply nested line
    // costs the reviewer daemon time quadratic in its depth -- and a file need
    // not compile to be diffed and scanned.
    let body = format!(
        "+tokio::spawn({}x{});",
        "g(".repeat(DEEP_NEST),
        ")".repeat(DEEP_NEST)
    );
    let diff = diff_of(&[("src/deep.rs", &body)]);

    let started = std::time::Instant::now();
    let report = run(&diff);
    let elapsed = started.elapsed();

    assert_eq!(
        report.tasks_scanned, 1,
        "the fixture has to reach the walk it is measuring. Summary was: {}",
        report.summary
    );
    assert!(
        elapsed < LINEAR_CEILING,
        "one line nested {DEEP_NEST} deep took {elapsed:?}, over a ceiling of \
         {LINEAR_CEILING:?}. That is the signature of a per-character search of \
         the parenthesis stack, and the input is webhook-supplied text."
    );
}
