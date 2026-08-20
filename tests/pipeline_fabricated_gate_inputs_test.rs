//! Lane `pipeline-fabricated-gate-inputs`: the review pipeline manufactures the
//! measurements it then reports on.
//!
//! # The defect, restated from source
//!
//! `slo_canary_guard` and `ci_wallclock_ratchet` were made honest: with no data
//! source they report `GateStatus::NotMeasured` naming the missing source. The
//! *caller* was not. `src/webhook/pipelines/review.rs` still writes the inputs
//! for three gates and hands them to the guard, so the guard's green is a
//! statement about a literal in the caller, not about the pull request:
//!
//!   - gate 44 (`AutomatedCanaryAnalysis`), review.rs:312-317 --
//!     `baseline_samples: vec![10.0, 10.2, 9.9]` against
//!     `canary_samples: vec![10.1, 10.3, 10.0]`. `evaluate_canary_distributions`
//!     (statistical_engine.rs:58-71) fails only above a 10% mean increase; these
//!     two means differ by ~1%, fixed at compile time. The Mann-Whitney U-test
//!     the gate is named for is not implemented at all.
//!   - gate 50 (`StackedDiffsOrchestrator`), review.rs:348 --
//!     `evaluate_stack_synchronization(&[])`. The slice is empty on every PR,
//!     and `compute_stack_plan` (dag_manager.rs:27-38) returns
//!     `atomic_merge_ready: true` unconditionally anyway, so the gate is
//!     doubly unfailable.
//!   - gate 51 (`MicroBenchmarkRatchet`), review.rs:351-360 --
//!     `base_ns_per_op: 50.0` and `head_ns_per_op: 50.0`, `p99_cpu_cycles_base:
//!     100` and `p99_cpu_cycles_head: 100`. Self-identical operands: the
//!     percentage change is 0.0 on every execution. (The two cycle fields are
//!     never read by `evaluate_benchmark_diff` at all -- criterion_diff.rs:31-52
//!     touches only the ns fields -- so half the fabricated struct is inert
//!     decoration.)
//!
//! # Premortem -- how the fix can already have failed
//!
//! P1. The literals are deleted from `review.rs` but the guard still returns a
//!     pass, so the scorecard is unchanged and nothing was fixed.
//!     -> the `*_publishes_not_measured_*` tests.
//! P2. "Moved, not removed": the struct literal is pushed into a helper, a
//!     `const`, a `Default` impl or a sibling file in `src/webhook/pipelines/`
//!     that the caller still reads. Or kept in place and merely re-valued
//!     (`50.0` -> `47.3`, `10.0` -> `9.8`), which defeats any fixed needle list
//!     while leaving the gate exactly as unfailable.
//!     -> `..._constructs_no_fabricated_measurement_struct` (needles) AND
//!        `..._assigns_no_numeric_literal_measurement` (the general mechanism
//!        that survives re-valuing). Both are checks over source text, because
//!        no comment and no prompt can stop the next author reintroducing a
//!        literal -- only a mechanism can.
//! P3. Over-correction: with no data source the gates report `Failed`, so every
//!     PR in the fleet is accused of a latency regression nobody can reproduce.
//!     Absent evidence is not a pass AND not an accusation.
//!     -> `..._does_not_fabricate_an_accusation`.
//! P4. The honest `NotMeasured` is produced by the guard and then thrown away by
//!     the wiring. `evaluator.rs:453` and `:514` rebuild `GateStatus` from
//!     `aca_report.passed` / `microbench_report.passed`, exactly the pattern
//!     that was just removed for six other gates (see the comments at
//!     evaluator.rs:243-246 and :275-277). A guard-level test cannot see this.
//!     -> `evaluator_reads_these_gate_verdicts_instead_of_rebuilding_them`.
//! P5. The `NotMeasured` gate_id does not match the field name on
//!     `PreMergeCertificationReport` or has no fidelity-registry entry, so
//!     `unmeasured_gates` names a gate nobody can look up and merge admission
//!     fails to block for a reason no reader can resolve.
//!     -> `unmeasured_gate_ids_are_registered_and_may_not_claim_a_pass`.
//!
//! # Why every source scan strips `#[cfg(test)]`
//!
//! A previous test in this repository was satisfied by a call inside a
//! `#[cfg(test)]` module and reported green on a change that had not been made.
//! Fixture values a test supplies are legitimate -- that is precisely what a
//! real data source will supply later -- so the production half is the only half
//! that can carry the defect, and it is the only half scanned here. Every
//! assertion below, positive and negative, runs over `production_source()`.
//!
//! # Out of scope, reported rather than asserted
//!
//! `review.rs` also calls `evaluate_replay_parity(&[])` (gate 60) and
//! `evaluate_upgrade_train(&[])` (gate 61) with the same empty-slice literal.
//! These are the same defect class but were not in the verified scope of this
//! lane, so no test here asserts them; banning `(&[])` outright would turn this
//! file red for work nobody was asked to do.

use anvil::automated_canary::{AutomatedCanaryAnalysis, MetricDistribution};
use anvil::microbenchmark_ratchet::{MicroBenchmarkRatchet, MicrobenchmarkSample};
use anvil::stacked_diffs::StackedDiffsOrchestrator;
use serde_json::Value;
use std::path::{Path, PathBuf};

// -------------------------------------------------------------------------
// Source-reading helpers
// -------------------------------------------------------------------------

/// The production half of a source file: everything before the first
/// `#[cfg(test)]`.
fn production_source(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    match s.find("#[cfg(test)]") {
        Some(i) => s[..i].to_string(),
        None => s,
    }
}

/// Blanks the contents of double-quoted strings and drops `//` line comments and
/// `/* */` block comments, so a scan sees code only. Every removed character is
/// replaced one-for-one and newlines are preserved, so line numbers and columns
/// still line up with the file.
///
/// String state is carried ACROSS lines deliberately. `review.rs:470-475`
/// contains a backslash-continued multi-line `format!` whose body includes
/// `X-Anvil-Version: 0.1.0`; a per-line scanner reads that as an assigned
/// numeric literal and reports a defect that is not there.
///
/// Not tracked: raw strings (`r#"..."#`) and char literals containing a quote.
/// Neither appears in any file scanned by this test file (verified by grep), and
/// either could only hide a hit, never invent one. Stated rather than implied,
/// because the point of this lane is to not overclaim what a mechanism covers.
fn code_only(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if c == '\n' {
            in_line_comment = false;
            out.push('\n');
            continue;
        }
        if in_line_comment {
            out.push(' ');
            continue;
        }
        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
                out.push_str("  ");
            } else {
                out.push(' ');
            }
            continue;
        }
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
                out.push('"');
                continue;
            }
            out.push(' ');
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push('"');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            in_line_comment = true;
            out.push(' ');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
            out.push_str("  ");
            continue;
        }
        out.push(c);
    }
    out
}

/// Every `.rs` file under a directory, as (repo-relative path, production half
/// with strings and comments blanked).
///
/// The directory rather than the single file, because the cheapest evasion of
/// P2 is moving the struct literal one file sideways into a
/// `fn default_distribution()` the caller still calls. The scan follows the
/// constant.
fn pipeline_sources() -> Vec<(String, String)> {
    let module_dir = "src/webhook/pipelines";
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(module_dir);
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = format!(
                    "{}/{}",
                    module_dir,
                    path.strip_prefix(&root).expect("under root").display()
                );
                let src = code_only(&production_source(&rel));
                out.push((rel, src));
            }
        }
    }
    assert!(
        !out.is_empty(),
        "no sources found under src/webhook/pipelines -- the scan silently covered nothing"
    );
    out.sort();
    out
}

/// Numeric literals *assigned* into a variable or a struct field, or written
/// inside an array literal, in a given line of code.
///
/// This is the generalisation of the needle lists: a needle list catches the
/// eight constants named in the brief, this catches the ninth. Re-valuing
/// `50.0` to `47.3` satisfies every needle list and leaves the gate exactly as
/// unfailable.
///
/// Deliberately narrow, so it cannot become the reason a real implementation is
/// blocked and the check deleted:
///   - string and comment contents are already blanked by `code_only`;
///   - comparisons (`>=`, `<=`, `==`, `!=`) are excluded -- a threshold compared
///     against a *measured* value is legitimate, a fabricated input is not;
///   - `= 0` and `= 1` are excluded -- initialising a counter is not a
///     measurement;
///   - function call arguments are NOT flagged, only `name: N`, `name = N` and
///     `[N, N, N]` forms.
fn assigned_numeric_literals(line: &str) -> Vec<String> {
    let code: Vec<char> = line.chars().collect();
    let mut hits = Vec::new();
    for i in 0..code.len() {
        if !code[i].is_ascii_digit() {
            continue;
        }
        // Only the first character of a literal, and never part of an
        // identifier such as `burn_rate_1h`, `p99_cpu_cycles` or `self.0`.
        if i > 0 {
            let p = code[i - 1];
            if p.is_ascii_digit() || p == '.' || p == '_' || p.is_alphabetic() {
                continue;
            }
        }
        let mut j = i;
        if j > 0 && code[j - 1] == '-' {
            j -= 1;
        }
        while j > 0 && code[j - 1] == ' ' {
            j -= 1;
        }
        if j == 0 {
            continue;
        }
        let prev = code[j - 1];
        // `[` and `,` cover array-literal elements: `vec![10.0, 10.2, 9.9]`,
        // where the number is not preceded by `:` or `=` at all.
        if prev != ':' && prev != '=' && prev != '[' && prev != ',' {
            continue;
        }
        if prev == '=' && j >= 2 && matches!(code[j - 2], '>' | '<' | '=' | '!') {
            continue;
        }
        let lit: String = code[i..]
            .iter()
            .take_while(|c| c.is_ascii_digit() || **c == '.' || **c == '_')
            .collect();
        if prev == '=' && (lit == "0" || lit == "1") {
            continue;
        }
        // `,` and `[` only count when the enclosing bracket is an array/vec
        // literal, which in practice means the line already opened one.
        if (prev == ',' || prev == '[') && !line.contains('[') {
            continue;
        }
        hits.push(format!("`{}` in `{}`", lit, line.trim()));
        break;
    }
    hits
}

// -------------------------------------------------------------------------
// Runtime helpers
// -------------------------------------------------------------------------

/// The `status` a gate report publishes, read through serde rather than through
/// a typed field.
///
/// `AutomatedCanaryReport`, `MicrobenchmarkReport` and `StackedDiffsReport`
/// carry no `GateStatus` today -- only a `passed: bool` -- so a typed
/// `rep.status` would not compile and this file could not be run at all before
/// the fix. Reading the serialized form lets the test FAIL rather than fail to
/// build, and pins the same contract `SloCanaryReport` and `CiWallclockReport`
/// already meet: a field named `status` holding a `GateStatus`.
fn published_status(report: &impl serde::Serialize, gate: &str) -> Value {
    let json = serde_json::to_value(report).expect("gate report must serialize");
    json.get("status").cloned().unwrap_or_else(|| {
        panic!(
            "{gate}: the report publishes no `status` field, so it cannot report \
             NotMeasured at all. `SloCanaryReport` and `CiWallclockReport` both carry \
             `status: GateStatus`; this report carries only a boolean, and a boolean \
             has no way to say \"nothing was measured\". Serialized report: {json}"
        )
    })
}

/// Asserts a published status is `NotMeasured`, with a gate_id matching the
/// `PreMergeCertificationReport` field name and a reason that names the missing
/// data source (not merely the absence of one).
fn assert_not_measured(status: &Value, gate: &str, expect_gate_id: &str, must_name: &[&str]) {
    let inner = status.get("NotMeasured").unwrap_or_else(|| {
        panic!(
            "{gate}: with no data source the gate must report \
             GateStatus::NotMeasured. `Passed` makes absent evidence a pass; \
             `Failed` fabricates an accusation. Got: {status}"
        )
    });
    let gate_id = inner
        .get("gate_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{gate}: NotMeasured carries no gate_id: {inner}"));
    assert_eq!(
        gate_id, expect_gate_id,
        "{gate}: gate_id must match the PreMergeCertificationReport field name, so \
         `unmeasured_gates` names a gate a human can look up"
    );
    let reason = inner
        .get("reason")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{gate}: NotMeasured carries no reason: {inner}"));
    let lower = reason.to_lowercase();
    assert!(
        must_name.iter().any(|n| lower.contains(&n.to_lowercase())),
        "{gate}: the reason must NAME the missing source (one of {must_name:?}), the way \
         slo_canary_guard names Prometheus and ci_wallclock_ratchet names the Actions \
         timing API. A reason that only says \"not configured\" tells the reader nothing \
         about what would close the gap. Got: {reason}"
    );
}

fn assert_no_accusation(status: &Value, gate: &str) {
    let is_accusation = status.get("Failed").is_some() || status.get("Errored").is_some();
    assert!(
        !is_accusation,
        "{gate}: a gate with no data source must not accuse a clean PR of a regression \
         nobody can reproduce. Got: {status}"
    );
}

// =========================================================================
// 1. Source: the caller must stop manufacturing its own inputs
// =========================================================================

/// Catches: the fabricated `MetricDistribution` and `MicrobenchmarkSample`
/// struct literals surviving anywhere in the pipeline module, including moved
/// one file sideways or hidden behind a helper the caller still calls (P2).
///
/// Fails today at `review.rs:312-317` and `:351-356`.
///
/// Why prompting would not prevent this: the instruction "do not fabricate gate
/// inputs" is invisible at the moment someone adds gate 71 and needs *something*
/// to pass to it. The literal is the path of least resistance and reads as
/// perfectly ordinary code; nothing in review or in CI objects. Only a check
/// that reads the source objects. This is also why the scan covers the whole
/// module directory rather than the two known line numbers.
#[test]
fn pipeline_constructs_no_fabricated_measurement_struct() {
    const BANNED: &[&str] = &[
        "MetricDistribution {",
        "MicrobenchmarkSample {",
        "baseline_samples",
        "canary_samples",
        "base_ns_per_op",
        "head_ns_per_op",
        "p99_cpu_cycles_base",
        "p99_cpu_cycles_head",
    ];

    let mut offenders: Vec<String> = Vec::new();
    for (rel, src) in pipeline_sources() {
        for (n, line) in src.lines().enumerate() {
            for banned in BANNED {
                if line.contains(banned) {
                    offenders.push(format!("{rel}:{}: `{}`", n + 1, line.trim()));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "False Green prevention: the review pipeline still writes the measurements it \
         then reports on. A gate handed a literal cannot fail, so its green describes \
         the literal, not the PR. These sites must be replaced by GateStatus::NotMeasured \
         naming the missing source, as slo_canary_guard and ci_wallclock_ratchet already \
         do. {} site(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// Catches: re-valuing instead of removing (P2). Swapping `50.0` for `47.3` and
/// `vec![10.0, 10.2, 9.9]` for `vec![9.8, 10.1, 9.7]` satisfies every needle in
/// the test above and leaves both gates exactly as unfailable.
///
/// The rule this encodes: `src/webhook/pipelines/` is the code that is supposed
/// to OBTAIN measurements, not to state them. A measurement arrives from a data
/// source; a numeric literal assigned into a field is the absence of one.
///
/// Fails today on the four `MicrobenchmarkSample` fields and the six sample
/// values inside the two `vec![]`s.
///
/// Why prompting would not prevent this: "remove the fabricated constants" is
/// satisfiable, in good faith, by changing the numbers to more plausible ones --
/// the reviewer sees different values and reads it as a real fix. A mechanism
/// over the source text cannot be satisfied that way.
#[test]
fn pipeline_assigns_no_numeric_literal_measurement() {
    let mut offenders: Vec<String> = Vec::new();
    for (rel, src) in pipeline_sources() {
        for (n, line) in src.lines().enumerate() {
            for hit in assigned_numeric_literals(line) {
                offenders.push(format!("{rel}:{}: {hit}", n + 1));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "False Green prevention: {} numeric literal(s) assigned as gate input in \
         production pipeline code, each a measurement nobody took:\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

/// Catches: gate 50 being fed a permanently empty stack (`review.rs:348`).
///
/// `evaluate_stack_synchronization(&[])` passes a literal that is empty on every
/// pull request forever. `compute_stack_plan` then returns
/// `atomic_merge_ready: true` unconditionally (dag_manager.rs:36), so the gate
/// is unfailable twice over: no input can reach it, and no input would change
/// the answer if it could.
///
/// Why prompting would not prevent this: `&[]` is not a suspicious-looking
/// value. It reads as "no stacked PRs here", which is true of most PRs, and the
/// reviewer's eye slides over it. Only naming the exact call in a check makes it
/// visible.
#[test]
fn pipeline_feeds_no_empty_stack_literal_to_the_stacked_diffs_gate() {
    let mut offenders: Vec<String> = Vec::new();
    for (rel, src) in pipeline_sources() {
        for (n, line) in src.lines().enumerate() {
            let squashed: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            if squashed.contains("evaluate_stack_synchronization(&[])") {
                offenders.push(format!("{rel}:{}: `{}`", n + 1, line.trim()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "False Green prevention: the stacked-diffs gate is called with an empty slice \
         literal, so it evaluates nothing on every PR:\n{}",
        offenders.join("\n")
    );
}

// =========================================================================
// 2. Behaviour: with no source configured, these gates must say so
// =========================================================================
//
// Each of these calls today's signature. If the fix drops the now-unused
// argument -- as `ci_wallclock_ratchet` did -- the call here needs updating; the
// durable half of each test is the assertion, not the call shape.

/// Catches P1: the literals are deleted from the caller but gate 44 still
/// returns a pass, so the scorecard is unchanged and nothing was actually fixed.
///
/// Fails today for a stronger reason than a wrong status: the report has no
/// `status` field at all, only `passed: bool`. And a boolean is exactly the
/// problem -- `evaluate_canary_distributions` returns `CanaryVerdict::Pass` when
/// both sample vectors are EMPTY (statistical_engine.rs:35-37), so "no data"
/// and "no regression" are the same value today. That is the inversion this
/// lane exists to remove.
///
/// Why prompting would not prevent this: the fabrication is in the caller, so
/// "delete the constants in review.rs" is a complete-sounding instruction that
/// leaves the guard reporting a pass on nothing. Only asserting on the guard's
/// published status closes that gap.
#[test]
fn automated_canary_publishes_not_measured_without_a_metrics_source() {
    let empty = MetricDistribution {
        metric_name: "p99_latency_ms".to_string(),
        baseline_samples: Vec::new(),
        canary_samples: Vec::new(),
    };
    let report = AutomatedCanaryAnalysis::new().evaluate_canary(&empty);
    let status = published_status(&report, "gate 44 AutomatedCanaryAnalysis");
    assert_not_measured(
        &status,
        "gate 44 AutomatedCanaryAnalysis",
        "automated_canary_status",
        &[
            "prometheus",
            "opentelemetry",
            "telemetry",
            "canary deployment",
            "metrics",
        ],
    );
}

/// Catches P3 for gate 44: over-correcting into a `Failed`, which would accuse
/// every PR in the fleet of a latency regression that no one can reproduce.
/// Absent evidence is not a pass and not an accusation.
#[test]
fn automated_canary_does_not_fabricate_an_accusation() {
    let empty = MetricDistribution {
        metric_name: "p99_latency_ms".to_string(),
        baseline_samples: Vec::new(),
        canary_samples: Vec::new(),
    };
    let report = AutomatedCanaryAnalysis::new().evaluate_canary(&empty);
    let status = published_status(&report, "gate 44 AutomatedCanaryAnalysis");
    assert_no_accusation(&status, "gate 44 AutomatedCanaryAnalysis");
}

/// Catches P1 for gate 51: no criterion baseline exists anywhere in this repo,
/// so there is nothing to ratchet against and the gate must say that rather
/// than report `Optimal`.
///
/// The sample below is the one `review.rs:351-356` fabricates verbatim, and it
/// is deliberately self-identical: base equals head on both the ns and the cycle
/// fields, so `evaluate_benchmark_diff` computes a 0.0% change and returns
/// `Optimal` on every execution (criterion_diff.rs:36-49). Even a real
/// regression could not surface here, because the caller never reads a
/// benchmark.
///
/// Why prompting would not prevent this: `base_ns_per_op: 50.0` /
/// `head_ns_per_op: 50.0` looks like a reasonable neutral default, and the guard
/// beneath it is genuine arithmetic -- the code is correct in every local sense.
/// The defect is only visible if you ask where the 50.0 came from, which is a
/// question a mechanism asks every time and a human asks once.
#[test]
fn microbenchmark_ratchet_publishes_not_measured_without_a_criterion_baseline() {
    let sample = MicrobenchmarkSample {
        benchmark_name: "hotpath_throughput".to_string(),
        base_ns_per_op: 50.0,
        head_ns_per_op: 50.0,
        p99_cpu_cycles_base: 100,
        p99_cpu_cycles_head: 100,
    };
    let report = MicroBenchmarkRatchet::new().evaluate_benchmark_regression(&sample);
    let status = published_status(&report, "gate 51 MicroBenchmarkRatchet");
    assert_not_measured(
        &status,
        "gate 51 MicroBenchmarkRatchet",
        "microbench_status",
        &["criterion", "benchmark", "baseline"],
    );
}

/// Catches P3 for gate 51.
#[test]
fn microbenchmark_ratchet_does_not_fabricate_an_accusation() {
    let sample = MicrobenchmarkSample {
        benchmark_name: "hotpath_throughput".to_string(),
        base_ns_per_op: 50.0,
        head_ns_per_op: 50.0,
        p99_cpu_cycles_base: 100,
        p99_cpu_cycles_head: 100,
    };
    let report = MicroBenchmarkRatchet::new().evaluate_benchmark_regression(&sample);
    let status = published_status(&report, "gate 51 MicroBenchmarkRatchet");
    assert_no_accusation(&status, "gate 51 MicroBenchmarkRatchet");
}

/// Catches P1 for gate 50, the third verified fabrication site.
///
/// With no stack information available the orchestrator must report
/// `NotMeasured`, not `passed: true`. Today `compute_stack_plan` hardcodes
/// `atomic_merge_ready: true` (dag_manager.rs:36), so this gate returns a pass
/// for the empty slice the pipeline gives it AND for any real stack it might
/// ever be given.
///
/// Why prompting would not prevent this: the hardcoded `true` is one file away
/// from the caller, so someone fixing "the empty slice in review.rs" would wire
/// a real branch list into a function that ignores it and publish a green with
/// more confidence than before.
#[test]
fn stacked_diffs_publishes_not_measured_without_stack_information() {
    let report = StackedDiffsOrchestrator::new().evaluate_stack_synchronization(&[]);
    let status = published_status(&report, "gate 50 StackedDiffsOrchestrator");
    assert_not_measured(
        &status,
        "gate 50 StackedDiffsOrchestrator",
        "stacked_diffs_status",
        &["stack", "parent", "dag", "branch"],
    );
    assert_no_accusation(&status, "gate 50 StackedDiffsOrchestrator");
}

// =========================================================================
// 3. Wiring: an honest verdict must survive the trip to the report
// =========================================================================

fn evaluator_production_source() -> String {
    code_only(&production_source("src/pre_merge_guard/evaluator.rs"))
}

/// Catches P4, the failure mode the parent lane explicitly warned about: the
/// guard is made honest and the evaluator throws the honesty away.
///
/// `evaluator.rs:453`, `:505` and `:514` currently read
/// `let automated_canary_status = if aca_report.passed { GateStatus::Passed } else { ... }`
/// -- the same rebuild-from-a-boolean that published absent coverage evidence as
/// the accusation "Coverage NaN% is below requirement" and absent SLO evidence
/// as `Passed`. It was removed for six gates (evaluator.rs:243-246, :275-277)
/// and left in place for these three. A `GateStatus::NotMeasured` returned by
/// the guard collapses to `Passed` here, because `NotMeasured` is not `passed ==
/// false`.
///
/// Why prompting would not prevent this: the bug is in the wiring, not the
/// guard, and it is invisible from both ends. The guard's own unit test passes,
/// the caller's test passes, and the boolean rebuild reads as harmless
/// normalisation. It is the single most likely way for this entire lane to ship
/// green and change nothing.
#[test]
fn evaluator_reads_these_gate_verdicts_instead_of_rebuilding_them() {
    let src = evaluator_production_source();

    // (report binding in evaluator.rs, the gate it feeds)
    const GATES: &[(&str, &str)] = &[
        ("aca_report", "44 AutomatedCanaryAnalysis"),
        ("stacked_report", "50 StackedDiffsOrchestrator"),
        ("microbench_report", "51 MicroBenchmarkRatchet"),
    ];

    let mut rebuilt: Vec<String> = Vec::new();
    let mut unread: Vec<String> = Vec::new();
    for (binding, gate) in GATES {
        if src.contains(&format!("= if {binding}.passed")) {
            rebuilt.push(format!("gate {gate} (`{binding}.passed`)"));
        }
        if !src.contains(&format!("{binding}.status")) {
            unread.push(format!("gate {gate} (`{binding}.status`)"));
        }
    }

    assert!(
        rebuilt.is_empty(),
        "the evaluator rebuilds these gate verdicts from a boolean, which discards \
         NotMeasured and republishes absent evidence as Passed: {rebuilt:?}. Read the \
         gate's own verdict instead, as gates 11 and 14 now do."
    );
    assert!(
        unread.is_empty(),
        "the evaluator never reads these gates' own GateStatus: {unread:?}. A verdict \
         the wiring does not read cannot reach the report."
    );
}

/// Catches P5: a `NotMeasured` whose gate_id nobody can resolve.
///
/// The gate_id is the join key between the published status, the fidelity
/// registry and the `PreMergeCertificationReport` field. If gate 44 reports
/// `NotMeasured { gate_id: "canary" }`, `unmeasured_gates` blocks merge
/// admission naming a gate that appears in no registry and no report field, and
/// the person holding the blocked PR has no way to find out what is missing.
///
/// The second assertion runs the invariant the other way: `fidelity/mod.rs`
/// states that a gate at `Aspirational` fidelity must report `NotMeasured`, and
/// nothing enforced the converse. A registry entry quietly upgraded to
/// `Partial` while the gate still measures nothing makes `may_report_pass()`
/// true again and reopens the exact hole this lane closes.
///
/// Why prompting would not prevent this: picking a gate_id is a free choice made
/// in passing, and any string compiles. The mismatch only shows up on a real
/// blocked PR, in production, to someone who cannot act on it.
#[test]
fn unmeasured_gate_ids_are_registered_and_may_not_claim_a_pass() {
    for gate_id in ["automated_canary_status", "microbench_status"] {
        let entry = anvil::fidelity::registry::AUDITED_GATES
            .iter()
            .find(|e| e.gate_id == gate_id)
            .unwrap_or_else(|| {
                panic!(
                    "gate_id `{gate_id}` has no entry in the fidelity registry, so \
                     `unmeasured_gates` would name a gate nobody can look up. \
                     slo_status and ci_wallclock_status are both registered."
                )
            });
        assert!(
            !entry.fidelity.may_report_pass(),
            "{gate_id} must report NotMeasured, but the registry declares it {}; a gate \
             entitled to report a pass has to produce a measurement",
            entry.fidelity.label()
        );
        assert!(
            entry.blocked_on.is_some(),
            "{gate_id} has no data source and must name what it is blocked on, so the \
             gap is closable rather than merely admitted"
        );
    }
}
