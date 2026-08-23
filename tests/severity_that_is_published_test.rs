//! Lane: severity-that-is-published.
//!
//! # The defect
//!
//! A gate's severity is decided in two places that disagree, and the place that
//! wins is not the place that knows.
//!
//! 1. `brand_absence` declares itself advisory -- `WARN_ONLY` is `true` and
//!    `is_blocking` is `!WARN_ONLY`, so always `false` -- and
//!    `tests/brand_absence_gate_test.rs` pins that. `evaluator.rs` ignored the
//!    field and minted `GateStatus::Failed` on any new violation. Verified at
//!    the head this lane branched from: `scan_tree` finds 12 new violations in
//!    Anvil's own `src/migration/registry.rs`, so a gate that deliberately
//!    chose not to block was hard-blocking every certification run in the
//!    fleet, over a file no tenant author can edit.
//!
//! 2. `schema_compat_status` and `performance_concurrency_status` are capped at
//!    `Warning`, which `is_acceptable()`. That is defensible for what they
//!    detect. What is not defensible is where the warning went:
//!    `publish::scorecard::render` builds `findings` unconditionally and emits
//!    it only on the `else` branch of `if report.is_admissible()`. A `Warning`
//!    cannot reach the blocked branch by itself -- it is acceptable, so a
//!    report carrying only warnings certifies -- so a detection was computed,
//!    counted as a pass by `gate_counts()`, and then thrown away.
//!
//! This is not confined to the two capped gates. `trace_context_guard` chose
//! `Warning` over `Passed` *specifically* so its "NOTHING TO MEASURE" sentence
//! would not render as a bare tick, and wrote so in its own field doc: "A gate
//! that formats a sentence it knows is discarded is publishing the same
//! unmeasured assurance it exists to remove." The renderer discarded it anyway
//! on every green pull request. The existing test that claims to cover this,
//! `a_diff_the_gate_found_no_boundary_in_says_so_on_the_scorecard`, builds its
//! fixture from `PreMergeCertificationReport::unmeasured(..)` -- every other
//! gate `NotMeasured`, so the report is inadmissible and the *blocked* branch
//! renders. It has never exercised the certified path.
//!
//! # Why prompting would not prevent this
//!
//! Both halves are decouplings, not lapses. The module that computes a
//! severity and the wiring that publishes it are different files; nothing in
//! the compiler relates `is_blocking: false` to a `GateStatus::Failed`
//! constructed two hundred lines away in another module. And a renderer that
//! *builds* a findings vector on both branches and *emits* it on one reads as
//! correct at every point a reviewer looks: the vector is populated, the
//! `Warning` arm is handled, the match is exhaustive. The value is dropped by
//! an `if` whose two arms were written months apart.

use anvil::brand_absence::{BrandAbsenceGate, BrandAbsenceReport, WARN_ONLY};
use anvil::pre_merge_guard::MatrixRenderer;
use anvil::pre_merge_guard::report::{GateStatus, PreMergeCertificationReport, TOTAL_GATES};
use anvil::pre_merge_guard::scanner::PreMergeScanner;
use anvil::publish;

/// GitHub rejects an issue-comment body above this length. Same constant as
/// `tests/scorecard_wiring_test.rs`; the budget is a property of GitHub, not of
/// either test file.
const GITHUB_COMMENT_LIMIT: usize = 65_536;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn corpus_gate_names() -> Vec<&'static str> {
    PreMergeCertificationReport::unmeasured("fixture baseline")
        .named_statuses()
        .into_iter()
        .map(|(name, _)| name)
        .collect()
}

/// A report built the way the spec suite builds one -- by handing an outcome
/// for every gate in the corpus to the constructor that consumes them -- with
/// `f` deciding each gate's status. Sealed, so the verdict and the unmeasured
/// list are derived rather than asserted.
fn report_where(f: impl Fn(&str) -> GateStatus) -> PreMergeCertificationReport {
    let outcomes: Vec<(&str, GateStatus)> =
        corpus_gate_names().into_iter().map(|n| (n, f(n))).collect();
    let mut r = PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("the fixture hands over an outcome for every gate in the corpus");
    r.recompute_unmeasured();
    r.seal();
    r.summary_markdown = MatrixRenderer::render(&r);
    r
}

fn all_passing() -> PreMergeCertificationReport {
    report_where(|_| GateStatus::Passed)
}

/// Every gate passing except `gate`, which warns with `sentence`.
fn certified_with_one_warning(gate: &str, sentence: &str) -> PreMergeCertificationReport {
    let r = report_where(|n| {
        if n == gate {
            GateStatus::Warning(sentence.to_string())
        } else {
            GateStatus::Passed
        }
    });
    assert!(
        r.is_admissible(),
        "fixture sanity: a report whose only non-pass is a Warning must certify, \
         or this suite is testing the blocked branch by accident"
    );
    r
}

/// The scorecard name a gate id renders under: `schema_compat_status` ->
/// `schema-compat`. Derived the same way `publish::scorecard` derives it, so a
/// change to that mapping does not leave this file asserting an old spelling.
fn gate_name(gate_id: &str) -> String {
    gate_id
        .strip_suffix("_status")
        .unwrap_or(gate_id)
        .replace('_', "-")
}

// ---------------------------------------------------------------------------
// 1. brand_absence: the published severity is the module's, not the wiring's
// ---------------------------------------------------------------------------

/// Catches: the evaluator overriding a module that declared itself advisory.
///
/// The severity has to be decided once, by the module that knows whether its
/// detection is precise enough to block. This asserts the module answers the
/// question at all -- today the answer lives only in `evaluator.rs`, where
/// `is_blocking` is not consulted.
#[test]
fn a_gate_that_declares_itself_advisory_publishes_an_advisory_status() {
    let gate = BrandAbsenceGate::with_allowlist(vec![]);
    let report = gate.scan_source(
        "src/synthetic/new_module.rs",
        "info!(\"hyperscaler-grade enterprise pipeline\");\n",
    );

    assert!(
        !report.new_violations.is_empty(),
        "fixture sanity: the scan must find the seeded violation, or the \
         severity below is the severity of nothing"
    );
    assert!(
        !report.is_blocking,
        "the module ships warn-only; `tests/brand_absence_gate_test.rs` pins it"
    );

    assert!(
        matches!(report.gate_status(), GateStatus::Warning(_)),
        "a module whose `is_blocking` is false must publish an acceptable \
         status. It published {:?}",
        report.gate_status()
    );
    assert!(
        report.gate_status().is_acceptable(),
        "an advisory finding must not withhold a merge"
    );
}

/// Catches: a status that is advisory and *empty* -- the other way to lose the
/// finding. A `Warning` carrying no sentence renders as a line with nothing
/// after the colon, which is worse than the `Failed` it replaces.
#[test]
fn the_advisory_status_carries_the_count_and_the_sentence() {
    let gate = BrandAbsenceGate::with_allowlist(vec![]);
    let report = gate.scan_source(
        "src/synthetic/new_module.rs",
        "info!(\"hyperscaler-grade enterprise pipeline\");\n",
    );

    let GateStatus::Warning(sentence) = report.gate_status() else {
        panic!(
            "expected an advisory status, got {:?}",
            report.gate_status()
        );
    };
    assert_eq!(
        sentence,
        "2 site(s) in Anvil's own tree, 2 occurrence(s) in all, stamp an aspiration \
         instead of naming what the code verifies",
        "the sentence must state what was found, in full. `contains(\"1\")` on a \
         one-violation fixture is satisfied by almost any string"
    );
}

/// Catches: the published count counting occurrences and calling them findings.
///
/// `new_violations` is one entry per occurrence because the ledger in `finish`
/// spends a per-`(path, stamp)` occurrence ceiling down one hit at a time. The
/// sentence is not the ledger, and on this repository's own tree the difference
/// is a factor of two: twelve occurrences at six sites, five of them the same
/// stamp repeated inside one string literal on one line, every one of them
/// carrying byte-identical evidence. That sentence is now composed in
/// `gate_status` and rendered on every certified pull request in the fleet, so
/// the number in it is this gate's to stand behind.
#[test]
fn the_advisory_sentence_counts_sites_not_repeated_occurrences() {
    let gate = BrandAbsenceGate::with_allowlist(vec![]);
    let report = gate.scan_source(
        "src/synthetic/new_module.rs",
        "info!(\"hyperscaler hyperscaler hyperscaler\");\n",
    );

    assert_eq!(
        report.new_violations.len(),
        3,
        "fixture sanity: one line, one stamp, three occurrences -- got {:?}",
        report.new_violations
    );

    let GateStatus::Warning(sentence) = report.gate_status() else {
        panic!(
            "expected an advisory status, got {:?}",
            report.gate_status()
        );
    };
    assert!(
        sentence.starts_with("1 site(s) in Anvil's own tree, 3 occurrence(s) in all,"),
        "one line carrying one stamp three times is one finding a reader can go \
         and look at, not three: {sentence}"
    );
}

/// Catches: a clean scan publishing a warning. A gate that warns on every run
/// is a gate that is switched off within a week -- Tricorder's finding, and the
/// reason its analyzers are held below a 10% false-positive rate.
#[test]
fn a_clean_scan_publishes_a_pass_not_a_standing_warning() {
    let gate = BrandAbsenceGate::with_allowlist(vec![]);
    let report = gate.scan_source("src/synthetic/clean.rs", "pub fn ok() -> usize { 0 }\n");

    assert!(report.new_violations.is_empty(), "fixture sanity");
    assert!(
        matches!(report.gate_status(), GateStatus::Passed),
        "a scan that found nothing has no finding to publish, got {:?}",
        report.gate_status()
    );
}

/// Catches: the severity being frozen at advisory rather than *delegated* to
/// the module's own switch. If `WARN_ONLY` is ever flipped -- which the module
/// documents as the way to make new violations fatal -- the status must follow
/// it without a second edit in the evaluator, or the two places disagree again
/// in the opposite direction.
#[test]
fn the_status_follows_the_modules_own_blocking_switch() {
    let gate = BrandAbsenceGate::with_allowlist(vec![]);
    let report = gate.scan_source(
        "src/synthetic/new_module.rs",
        "info!(\"hyperscaler-grade enterprise pipeline\");\n",
    );

    // Read from the report's own field rather than from `WARN_ONLY` directly,
    // so the assertion is about the wiring between them.
    assert_eq!(
        report.is_blocking, !WARN_ONLY,
        "fixture sanity: `is_blocking` is derived from `WARN_ONLY`"
    );
    assert!(
        matches!(report.gate_status(), GateStatus::Warning(_)),
        "warn-only must publish an acceptable status, got {:?}",
        report.gate_status()
    );

    // The other arm. `WARN_ONLY` is a `const`, so the blocking case is
    // unreachable by scanning -- and a `gate_status` that returned `Warning`
    // unconditionally would satisfy every other assertion in this file while
    // being exactly the defect again, one flag flip later. The struct's fields
    // are public, so the arm is reached directly rather than left untested.
    let blocking = BrandAbsenceReport {
        is_blocking: true,
        ..report.clone()
    };
    assert!(
        matches!(blocking.gate_status(), GateStatus::Failed(_)),
        "a report that says it blocks must publish a blocking status, got {:?}",
        blocking.gate_status()
    );
    assert!(
        !blocking.gate_status().is_acceptable(),
        "a blocking severity that is acceptable withholds nothing"
    );
}

/// Catches: the evaluator writing the verdict back by hand. The repository's
/// rule is that a gate owning a `GateStatus` has it read, not rebuilt --
/// `tests/evaluator_preserves_gate_verdicts_test.rs` enforces the shape of the
/// line for every gate on its list. This pins the negative: no
/// `GateStatus::Failed` may be constructed for this gate anywhere in the
/// wiring, which is the exact edit that produced the defect.
#[test]
fn the_evaluator_constructs_no_severity_for_the_brand_absence_gate() {
    let src = include_str!("../src/pre_merge_guard/evaluator.rs");

    let bindings: Vec<&str> = src
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("let brand_absence_status"))
        .collect();

    assert_eq!(
        bindings,
        vec!["let brand_absence_status = brand_absence_report.gate_status();"],
        "the verdict must be read from the gate in the one shape \
         `tests/evaluator_preserves_gate_verdicts_test.rs` accepts. Any other \
         binding -- notably the `= {{` block this lane deleted -- is the wiring \
         deciding a severity the module already decided"
    );
}

// ---------------------------------------------------------------------------
// 2. The renderer: an acceptable finding is still a finding
// ---------------------------------------------------------------------------

/// Catches: the headline defect. A gate detected something, the report
/// certified because the detection is advisory, and the scorecard published
/// nothing about it.
#[test]
fn a_warning_on_a_certified_pull_request_is_published() {
    let sentence = "Destructive schema migration detected (DROP/NOT NULL without multi-phase \
                    rollout). Verify backwards compatibility across cell nodes.";
    let report = certified_with_one_warning("schema_compat_status", sentence);

    let body = publish::scorecard::render(&report);

    assert!(
        body.contains(sentence),
        "the gate composed a sentence and the scorecard published none of it:\n{body}"
    );
    assert!(
        body.contains("**schema-compat**"),
        "the finding must name its gate, or a reader cannot tell which of \
         {TOTAL_GATES} found something:\n{body}"
    );
}

/// Catches: the fix being applied to one gate's row rather than to the
/// renderer. Every gate that can warn is affected, so the property is asserted
/// over the corpus rather than over the gate that happened to prompt the lane.
#[test]
fn every_gate_that_warns_reaches_the_reader_on_a_certified_report() {
    let mut missing: Vec<String> = Vec::new();

    for gate_id in corpus_gate_names() {
        let sentence = format!("advisory finding raised by {gate_id}");
        let report = certified_with_one_warning(gate_id, &sentence);
        let body = publish::scorecard::render(&report);
        if !body.contains(&sentence) || !body.contains(&format!("**{}**", gate_name(gate_id))) {
            missing.push(gate_id.to_string());
        }
    }

    assert!(
        missing.is_empty(),
        "{} of {TOTAL_GATES} gates can warn on a certified pull request and \
         have the warning discarded: {missing:?}",
        missing.len()
    );
}

/// Catches: the two capped gates specifically, driven through the real scanner
/// rather than through a hand-written `GateStatus`. A test that asserts on a
/// literal it wrote itself proves the renderer prints strings; this proves the
/// path from a seeded diff to the published comment.
#[test]
fn a_real_detection_by_either_capped_gate_reaches_the_published_comment() {
    let schema_diff = "diff --git a/db/migrations/003_drop.sql b/db/migrations/003_drop.sql\n\
                       +++ b/db/migrations/003_drop.sql\n\
                       +ALTER TABLE tenants DROP COLUMN legacy_region;\n";
    let schema_status = PreMergeScanner::scan_for_breaking_changes(
        schema_diff,
        &["db/migrations/003_drop.sql".to_string()],
    );

    let flake_diff = "diff --git a/src/x.rs b/src/x.rs\n\
                      +++ b/src/x.rs\n\
                      +    thread::sleep(Duration::from_millis(250));\n";
    let flake_status = PreMergeScanner::scan_for_concurrency_and_flakes(flake_diff);

    for (gate_id, status) in [
        ("schema_compat_status", schema_status),
        ("performance_concurrency_status", flake_status),
    ] {
        let GateStatus::Warning(sentence) = status.clone() else {
            panic!("fixture sanity: {gate_id} did not detect the seeded defect, got {status:?}");
        };

        let report = certified_with_one_warning(gate_id, &sentence);
        let body = publish::scorecard::render(&report);

        assert!(
            body.contains(&sentence),
            "{gate_id} detected a real defect on a pull request that certifies, \
             and the reader was told nothing:\n{body}"
        );
        assert!(
            body.contains(&format!("**{}**", gate_name(gate_id))),
            "{gate_id}'s finding must name the gate:\n{body}"
        );
    }
}

/// Catches: the sentence reaching the page under a verdict that contradicts it.
/// P9 of the scorecard lane -- "a clean PR is published as blocked ... and the
/// gate gets bypassed" -- applies with equal force to an advisory finding
/// rendered as a failure.
#[test]
fn an_advisory_finding_does_not_turn_the_verdict_red() {
    let report = certified_with_one_warning("performance_concurrency_status", "a timing risk");
    let body = publish::scorecard::render(&report);

    assert!(
        body.contains(&format!(
            "✅ Certified — {TOTAL_GATES}/{TOTAL_GATES} gates passed."
        )),
        "the verdict line must be unchanged; the pull request is certified:\n{body}"
    );
    assert!(
        !body.contains("❌ Blocked"),
        "an acceptable finding must not read as a blocked merge:\n{body}"
    );
    assert!(
        !body.contains("finding(s) across"),
        "the blocked header must not appear on a certified scorecard:\n{body}"
    );
}

/// Catches: the advisory block being rendered without saying what it is. A
/// finding under no heading, on a page headed "Certified", reads as a defect
/// that blocked nothing for no stated reason -- which is how a reader learns to
/// skip it. The published text has to say the gate is advisory, since the
/// registry it points at is not on the page.
#[test]
fn the_advisory_block_states_that_it_does_not_block_and_counts_itself() {
    let report = report_where(|n| match n {
        "schema_compat_status" | "performance_concurrency_status" => {
            GateStatus::Warning(format!("advisory finding raised by {n}"))
        }
        _ => GateStatus::Passed,
    });
    assert!(report.is_admissible(), "fixture sanity");

    let body = publish::scorecard::render(&report);
    let lower = body.to_lowercase();

    assert!(
        lower.contains("advisory"),
        "the block must say the findings are advisory:\n{body}"
    );
    assert!(
        lower.contains("not blocking") || lower.contains("does not block"),
        "the block must say plainly that these findings did not withhold the \
         merge:\n{body}"
    );
    assert!(
        body.contains("2 advisory"),
        "the count has to be stated, so two findings are not read as one:\n{body}"
    );
    assert!(
        body.contains("do not fully measure"),
        "the fidelity disclosure #60 put on the certified path must survive \
         alongside the advisory block; both describe the same green verdict:\n{body}"
    );
}

/// Catches: the other direction. Moving warnings onto the certified branch
/// must not take them off the blocked one -- a report carrying a failure and a
/// warning has to publish both, and the warning must still read as advisory
/// rather than as a second reason the merge was withheld.
#[test]
fn a_blocked_scorecard_still_carries_the_warnings_it_carried_before() {
    let report = report_where(|n| match n {
        "cedar_status" => GateStatus::Failed("no policy covers POST /v1/tenants".to_string()),
        "performance_concurrency_status" => {
            GateStatus::Warning("Concurrency/Timing Warning: a real-clock sleep".to_string())
        }
        _ => GateStatus::Passed,
    });
    assert!(
        !report.is_admissible(),
        "fixture sanity: this one is blocked"
    );

    let body = publish::scorecard::render(&report);
    assert!(body.contains("❌ Blocked"), "fixture sanity:\n{body}");
    assert!(
        body.contains("Concurrency/Timing Warning: a real-clock sleep"),
        "the warning was published before this lane and must still be:\n{body}"
    );
    assert!(
        body.contains("no policy covers POST /v1/tenants"),
        "the failure must still be published:\n{body}"
    );
}

/// Catches: the trace gate's documented intent, on the path it was written for.
///
/// `TraceContextReport::status` is `Warning` rather than `Passed` precisely so
/// the "NOTHING TO MEASURE" sentence would not render as a bare tick. The only
/// existing test of that renders it from an all-`NotMeasured` fixture, which
/// takes the blocked branch. On the branch a real green pull request takes, the
/// sentence was dropped.
#[test]
fn the_gate_that_measured_nothing_says_so_on_a_green_pull_request() {
    let sentence = "➖ NOTHING TO MEASURE (no task boundary in the changed Rust hunks)";
    let report = certified_with_one_warning("trace_status", sentence);
    assert!(
        report.is_certified_ready,
        "fixture sanity: this is the green path, not the blocked one"
    );

    let body = publish::scorecard::render(&report);
    assert!(
        body.contains(sentence),
        "a gate that inspected nothing was counted in {TOTAL_GATES}/{TOTAL_GATES} \
         and said nothing:\n{body}"
    );
}

// ---------------------------------------------------------------------------
// 3. Noise: what a passing scorecard looks like now
// ---------------------------------------------------------------------------

/// Catches: the change taxing every green pull request. Making an advisory
/// finding visible is only an improvement if it appears when there IS one. A
/// report with nothing to say must publish exactly what it published before.
#[test]
fn a_scorecard_with_no_findings_publishes_exactly_what_it_did_before() {
    let body = publish::scorecard::render(&all_passing());

    assert!(
        !body.to_lowercase().contains("advisory"),
        "a clean certified scorecard grew an empty advisory block:\n{body}"
    );
    let content_lines = body
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("<!--") && *l != "---")
        .count();
    assert!(
        content_lines <= 3,
        "the certified path is the common path and must not bury; \
         `tests/scorecard_wiring_test.rs` pins the same ceiling. Got \
         {content_lines} lines:\n{body}"
    );
}

/// Catches: the advisory block enumerating gates that passed -- the 68-row
/// table returning through a side door. Only gates with something to say may
/// appear.
#[test]
fn the_advisory_block_names_no_gate_that_passed() {
    let report = certified_with_one_warning("schema_compat_status", "a schema risk");
    let body = publish::scorecard::render(&report);

    let named: Vec<String> = corpus_gate_names()
        .into_iter()
        .filter(|g| *g != "schema_compat_status")
        .map(|g| format!("**{}**", gate_name(g)))
        .filter(|needle| body.contains(needle))
        .collect();

    assert!(
        named.is_empty(),
        "a passing gate produced a finding line; the scorecard's rule is \
         findings only: {named:?}\n{body}"
    );
}

// ---------------------------------------------------------------------------
// 4. The size budget, on the surface this lane changes
// ---------------------------------------------------------------------------

/// Catches: the certified path outgrowing what GitHub accepts, or outgrowing
/// the matrix it replaces.
///
/// `tests/scorecard_wiring_test.rs` pins the worst case for the *blocked*
/// branch. The certified branch had no worst case worth pinning while it
/// published one line; it does now, and its worst case is every gate in the
/// corpus warning at once -- which still certifies, because `Warning` is
/// acceptable.
#[test]
fn boundary_worst_case_certified_body_fits_a_comment_and_beats_the_table() {
    let report = report_where(|_| GateStatus::Warning("seeded advisory".to_string()));
    assert!(
        report.is_admissible(),
        "fixture sanity: a corpus of warnings certifies, which is what makes \
         this the certified path's worst case"
    );

    let body = publish::scorecard::render(&report);

    assert!(
        body.len() < GITHUB_COMMENT_LIMIT,
        "worst-case certified body is {} chars, above GitHub's {} limit",
        body.len(),
        GITHUB_COMMENT_LIMIT
    );
    let matrix = MatrixRenderer::render(&report);
    assert!(
        body.len() < matrix.len(),
        "worst-case certified body is {} chars against the matrix's {}; the \
         terse rendering must not be larger than what it replaces",
        body.len(),
        matrix.len()
    );
    assert!(
        body.contains(&format!("{TOTAL_GATES} advisory")),
        "every warning must be accounted for in the count:\n{}",
        &body[..body.len().min(400)]
    );
}
