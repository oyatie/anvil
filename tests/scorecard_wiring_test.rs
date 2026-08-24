//! Lane: scorecard-wiring.
//!
//! PREMORTEM. Assume the terse scorecard has already shipped and already failed
//! on a live pull request. The enumerated ways it failed, each turned into a
//! test below:
//!
//!  P1  The renderer was written but never wired: `upsert_pr_comment` still
//!      receives `cert_report.summary_markdown`, so production is unchanged
//!      while the renderer's own unit tests pass. Invisible in every test that
//!      calls `publish::scorecard::render` directly.
//!  P2  The HTML marker was dropped or altered while rewiring, so every review
//!      run posts a NEW comment instead of amending; the PR accumulates one
//!      scorecard per push.
//!  P3  The mandatory signature was lost, or no longer last, because the new
//!      body is assembled by hand instead of through `publish::body`.
//!  P4  Only the blocked path was rewired; a certified PR still gets the
//!      68-row table, so the burying this change exists to remove survives on
//!      the path that is taken most often.
//!  P5  Findings enumerate `Failed` only, silently dropping `Errored` and
//!      `NotMeasured`. Absent evidence is then published as a pass (I1).
//!  P6  `NotMeasured` is rendered as "failed", fabricating an accusation
//!      against a gate that never ran (the symmetric I1 violation).
//!  P7  The counts in the header come from a constant or from
//!      `is_certified_ready` rather than `gate_counts()`, reviving the
//!      hardcoded 69/70 (I2).
//!  P8  A report that certifies on every measured gate but has an unmeasured
//!      gate publishes as certified, hiding the reason merge admission is
//!      blocked.
//!  P9  A clean PR is published as blocked, or with remediation hints for
//!      gates that passed -- reviewers learn to ignore the comment, and the
//!      gate gets bypassed.
//! P10  `matrix.rs` is deleted or its callers broken. `evaluator.rs` still
//!      calls `MatrixRenderer::render_matrix` to populate `summary_markdown`,
//!      and `tests/red_green_gates_test.rs` asserts against it.
//! P11  Ordering is non-deterministic, so an amend rewrites the comment with
//!      reshuffled findings on every push and the diff is unreviewable.
//! P12  With many gates failing the body exceeds GitHub's 65536-character
//!      comment limit and the post is rejected outright.

use anvil::pre_merge_guard::{GateStatus, MatrixRenderer, PreMergeCertificationReport};
use anvil::publish;
use anvil::webhook::pipelines::review;

/// GitHub rejects an issue-comment body above this length.
const GITHUB_COMMENT_LIMIT: usize = 65_536;

/// Every gate in the corpus reporting `Passed`, built the way a report is
/// built: by handing the gate outcomes to the constructor that consumes them.
///
/// This was seventy-two lines of struct literal until `provenance` became a
/// private field — a `Copy` mark that any struct literal could lift off a
/// genuine report. The gate names are now read off the corpus rather than typed
/// out, so the fixture stays correct when the corpus grows.
fn all_passing() -> PreMergeCertificationReport {
    let base = PreMergeCertificationReport::unmeasured("fixture baseline");
    let names: Vec<&'static str> = base.named_statuses().into_iter().map(|(n, _)| n).collect();
    let outcomes: Vec<(&str, GateStatus)> =
        names.into_iter().map(|n| (n, GateStatus::Passed)).collect();
    let mut r = PreMergeCertificationReport::from_gate_outcomes(&outcomes)
        .expect("the fixture hands over an outcome for every gate in the corpus");
    r.recompute_unmeasured();
    r.summary_markdown = matrix_for(&r);
    r
}

/// Exactly what `evaluator.rs` puts in `summary_markdown`: the full table,
/// rendered from the report so the fixture is the real published body.
fn matrix_for(r: &PreMergeCertificationReport) -> String {
    MatrixRenderer::render(r)
}

/// Rebuilds the derived fields after mutating gate statuses, so a fixture can
/// never disagree with itself.
fn seal(r: &mut PreMergeCertificationReport) {
    let failed = r.gate_counts().failed;
    r.is_certified_ready = failed == 0;
    r.recompute_unmeasured();
    r.summary_markdown = matrix_for(r);
}

/// The `upsert_pr_comment` call that posts the scorecard, as a window of source
/// lines around the marker literal.
///
/// A file-wide substring check is not enough to enforce P1: a dead
/// `let _ = scorecard_comment(&cert_report);` next to a locally bound copy of
/// `summary_markdown` would satisfy it while production still published the
/// 68-row table. The assertion has to name the argument actually passed.
fn scorecard_upsert_call(src: &str) -> Vec<&str> {
    let lines: Vec<&str> = src.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains("\"<!-- ANVIL_SCORECARD_RECEIPT -->\""))
        .expect("the scorecard marker must appear at an upsert call site");
    let start = at.saturating_sub(8);
    let end = (at + 4).min(lines.len());
    lines[start..end].to_vec()
}

fn passed_row_count(body: &str) -> usize {
    body.matches("✅ PASSED").count()
}

// ---------------------------------------------------------------------------
// red -> green: the published body is the terse renderer, on both outcomes.
// ---------------------------------------------------------------------------

/// P1, P4. The body handed to `upsert_pr_comment` for a blocked PR must be the
/// findings-only rendering, not the 68-row matrix.
#[test]
fn blocked_scorecard_published_is_findings_only_not_the_68_row_matrix() {
    let mut r = all_passing();
    r.cedar_status = GateStatus::Failed("no policy covers POST /v1/tenants".into());
    r.coverage_status = GateStatus::Failed("62.0% is below the required 85%".into());
    seal(&mut r);

    let published = review::scorecard_comment(&r);

    assert!(
        !published.contains("| Quality Gate | Status | Details |"),
        "the published body must not be the gate table:\n{published}"
    );
    assert_eq!(
        passed_row_count(&published),
        0,
        "passing gates must be counted, never enumerated:\n{published}"
    );
    assert!(
        published.contains("- **cedar** — failed:"),
        "the failing gate must be named as a finding:\n{published}"
    );
    assert!(
        published.contains("- **coverage** — failed:"),
        "every failing gate must be named:\n{published}"
    );
}

/// P4. Success is one line. The certified path is the common path, so it is the
/// one that must not bury.
#[test]
fn certified_scorecard_published_is_a_single_counted_line() {
    let r = all_passing();
    let published = review::scorecard_comment(&r);

    assert!(
        published.contains(&format!(
            "✅ Certified — {n}/{n} gates passed.",
            n = anvil::pre_merge_guard::report::TOTAL_GATES
        )),
        "certified body must state real counts on one line:\n{published}"
    );
    assert!(
        !published.contains("| Quality Gate |"),
        "certified body must not enumerate gates:\n{published}"
    );
    let content_lines = published
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with("<!--") && *l != "---")
        .count();
    assert!(
        content_lines <= 3,
        "certified body should be a verdict line plus signature, got {content_lines}:\n{published}"
    );
}

// ---------------------------------------------------------------------------
// FALSE-GREEN PREVENTION: fixtures the wiring must reject, forever.
// ---------------------------------------------------------------------------

/// P1. Without this the change is indistinguishable from not having been made:
/// the renderer exists and is unit-tested while production still publishes the
/// table. Enforced by mechanism at the call site (I22), because the pipeline
/// function itself cannot be invoked without a live AppState.
#[test]
fn false_green_prevention_upsert_call_site_must_not_publish_summary_markdown() {
    let src = include_str!("../src/webhook/pipelines/review.rs");
    assert!(
        !src.contains("&cert_report.summary_markdown"),
        "Expected False Green prevention: the upsert path must stop publishing \
         the 68-row summary_markdown"
    );

    // Asserted at the argument position, not anywhere in the file: a call to
    // the renderer whose result is discarded is not wiring.
    let call = scorecard_upsert_call(src);
    let window = call.join("\n");
    assert!(
        window.contains("upsert_pr_comment("),
        "Expected False Green prevention: the marker must sit in an \
         upsert_pr_comment call, not somewhere else in the file:\n{window}"
    );
    assert!(
        !window.contains("summary_markdown"),
        "Expected False Green prevention: the 68-row matrix must not be the \
         body argument of the scorecard upsert:\n{window}"
    );
    assert!(
        window.contains("scorecard_comment(&cert_report)"),
        "Expected False Green prevention: the body argument of the scorecard \
         upsert must be the terse renderer's output:\n{window}"
    );
}

/// P5, I1. An `Errored` gate produced no measurement. It must appear as a
/// finding and it must block; dropping it publishes absent evidence as a pass.
#[test]
fn false_green_prevention_an_errored_gate_is_published_and_blocks() {
    let mut r = all_passing();
    r.slo_status = GateStatus::Errored("prometheus probe timed out after 60s".into());
    seal(&mut r);

    let published = review::scorecard_comment(&r);
    assert!(
        !published.contains("✅ Certified"),
        "Expected False Green prevention: a gate that could not measure must \
         never certify:\n{published}"
    );
    assert!(
        published.contains("- **slo** — errored: prometheus probe timed out"),
        "Expected False Green prevention: the errored gate must be named with \
         its reason:\n{published}"
    );
}

/// P7, I2. Vary the number of failures and require the printed counts to track
/// them. A constant header satisfies every single-fixture assertion.
#[test]
fn false_green_prevention_counts_track_reality_and_are_not_a_constant() {
    let clean = review::scorecard_comment(&all_passing());
    let n = anvil::pre_merge_guard::report::TOTAL_GATES;
    assert!(clean.contains(&format!("{n}/{n}")), "{clean}");

    let mut one = all_passing();
    one.cedar_status = GateStatus::Failed("policy gap".into());
    seal(&mut one);
    let one_body = review::scorecard_comment(&one);

    let mut three = all_passing();
    three.cedar_status = GateStatus::Failed("policy gap".into());
    three.coverage_status = GateStatus::Failed("below threshold".into());
    three.test_suite_status = GateStatus::Failed("3 tests failing".into());
    seal(&mut three);
    let three_body = review::scorecard_comment(&three);

    // Anchored on the verdict prefix: an unanchored `contains("1 finding(s)")`
    // is also satisfied by "21 finding(s)".
    assert!(
        one_body.contains("❌ Blocked — 1 finding(s)"),
        "Expected False Green prevention: one failure must render as one \
         finding:\n{one_body}"
    );
    assert!(
        three_body.contains("❌ Blocked — 3 finding(s)"),
        "Expected False Green prevention: three failures must render as three, \
         not the historical constant 1:\n{three_body}"
    );
    assert!(
        !one_body.contains("69/70") && !three_body.contains("69/70"),
        "Expected False Green prevention: the hardcoded 69/70 must not return"
    );
}

/// P8, I1. Every measured gate passes; one gate never ran. `is_certified_ready`
/// is true and `is_admissible` is false -- the published body must show the
/// second, not the first.
#[test]
fn false_green_prevention_an_unmeasured_gate_is_never_published_as_certified() {
    let mut r = all_passing();
    r.kani_status = GateStatus::NotMeasured {
        gate_id: "kani_status".into(),
        reason: "kani is not installed on this runner".into(),
    };
    r.recompute_unmeasured();
    r.is_certified_ready = true;
    r.summary_markdown = matrix_for(&r);

    assert!(r.is_certified_ready);
    assert!(!r.is_admissible());

    let published = review::scorecard_comment(&r);
    assert!(
        !published.contains("✅ Certified"),
        "Expected False Green prevention: absent evidence must not publish as a \
         pass:\n{published}"
    );
    assert!(
        published.contains("1 gate(s) produced no measurement"),
        "Expected False Green prevention: the unmeasured gate must be stated:\n{published}"
    );
}

/// P2. Changing the marker orphans every scorecard already posted, which then
/// duplicates instead of amending. Asserted on the body and at the call site.
#[test]
fn false_green_prevention_marker_leads_the_body_and_is_unchanged() {
    for r in [all_passing(), {
        let mut b = all_passing();
        b.cedar_status = GateStatus::Failed("policy gap".into());
        seal(&mut b);
        b
    }] {
        let published = review::scorecard_comment(&r);
        assert!(
            published.starts_with("<!-- ANVIL_SCORECARD_RECEIPT -->"),
            "Expected False Green prevention: the marker must lead so the \
             comment is amended in place:\n{published}"
        );
        assert_eq!(
            published
                .matches("<!-- ANVIL_SCORECARD_RECEIPT -->")
                .count(),
            1,
            "exactly one marker:\n{published}"
        );
    }
}

/// P3. The signature is mandatory and always last.
#[test]
fn false_green_prevention_every_published_scorecard_is_signed_last() {
    let mut blocked = all_passing();
    blocked.supply_chain_status = GateStatus::Failed("RUSTSEC-2024-0011 in time 0.1.44".into());
    seal(&mut blocked);

    for (label, r) in [("certified", all_passing()), ("blocked", blocked)] {
        let published = review::scorecard_comment(&r);
        assert!(
            publish::is_signed(&published),
            "Expected False Green prevention: {label} scorecard is unsigned:\n{published}"
        );
        let expected = if r.is_admissible() {
            publish::signature(publish::AnvilAction::Certified)
        } else {
            publish::signature(publish::AnvilAction::Blocked)
        };
        assert!(
            published.trim_end().ends_with(&expected),
            "Expected False Green prevention: signature must be last on the \
             {label} scorecard:\n{published}"
        );
    }
}

/// P11. An amended comment is a diff a human reads. Re-rendering the same
/// report must be byte-identical, and finding order must follow gate
/// declaration order rather than iteration order of a map.
#[test]
fn false_green_prevention_rendering_is_deterministic_and_ordered() {
    let mut r = all_passing();
    r.test_suite_status = GateStatus::Failed("3 tests failing".into());
    r.cedar_status = GateStatus::Failed("policy gap".into());
    r.coverage_status = GateStatus::Failed("below threshold".into());
    seal(&mut r);

    let a = review::scorecard_comment(&r);
    let b = review::scorecard_comment(&r);
    assert_eq!(
        a, b,
        "Expected False Green prevention: rendering must be stable"
    );

    let cedar = a.find("- **cedar**").expect("cedar finding");
    let coverage = a.find("- **coverage**").expect("coverage finding");
    let tests = a.find("- **test-suite**").expect("test-suite finding");
    assert!(
        cedar < coverage && coverage < tests,
        "Expected False Green prevention: findings must follow gate declaration \
         order:\n{a}"
    );
}

// ---------------------------------------------------------------------------
// FALSE-RED PREVENTION: clean fixtures that must keep passing.
// ---------------------------------------------------------------------------

/// P9. A clean PR must publish as certified, with no remediation hints and no
/// blocked verdict. A comment that nags on green work gets bypassed.
#[test]
fn false_red_prevention_a_clean_report_publishes_as_certified_with_no_hints() {
    let r = all_passing();
    let published = review::scorecard_comment(&r);
    assert!(
        !published.contains("❌ Blocked"),
        "Expected False Red prevention: a fully passing report must not be \
         blocked:\n{published}"
    );
    assert!(
        !published.contains("fix:"),
        "Expected False Red prevention: no remediation for gates that \
         passed:\n{published}"
    );
    assert!(
        !published.contains("— failed"),
        "Expected False Red prevention: no findings on a clean report:\n{published}"
    );
}

/// P9. A `Warning` is advisory: `is_acceptable()` is true for it, so it must
/// not turn a green PR red.
///
/// Scope note: the renderer this lane wires in deliberately publishes nothing
/// but the verdict line on the admissible path, so the advisory text itself is
/// not surfaced. Whether a warning should be visible on a certified scorecard
/// is a change to `publish::scorecard::render`, not to the wiring, and belongs
/// in its own commit (I8). This test asserts only what the lane owns: a warning
/// does not block.
#[test]
fn false_red_prevention_a_warning_does_not_block() {
    let mut r = all_passing();
    r.finops_status = GateStatus::Warning("2 new heap allocations in a hot path".into());
    seal(&mut r);

    assert!(r.is_admissible(), "a warning must not block admission");
    let published = review::scorecard_comment(&r);
    assert!(
        published.contains("✅ Certified"),
        "Expected False Red prevention: a warning must not block:\n{published}"
    );
    assert!(
        !published.contains("❌ Blocked"),
        "Expected False Red prevention: an advisory must not produce a blocked \
         verdict:\n{published}"
    );
    assert!(
        !published.contains("— failed"),
        "Expected False Red prevention: a warning must not be reported as a \
         failure:\n{published}"
    );
}

/// P10. `matrix.rs` is not deleted: `evaluator.rs` still calls it to populate
/// `summary_markdown`, and `red_green_gates_test.rs` asserts against its
/// output. Both must keep working.
#[test]
fn false_red_prevention_matrix_renderer_survives_for_its_remaining_callers() {
    let r = all_passing();
    let matrix = matrix_for(&r);
    assert!(
        matrix.contains("| Quality Gate | Status | Details |"),
        "Expected False Red prevention: MatrixRenderer must still render its table"
    );
    assert!(
        matrix.contains("<!-- ANVIL_SCORECARD_RECEIPT -->"),
        "Expected False Red prevention: red_green_gates_test.rs asserts this marker"
    );

    let evaluator = include_str!("../src/pre_merge_guard/evaluator.rs");
    assert!(
        evaluator.contains("MatrixRenderer::render("),
        "Expected False Red prevention: evaluator.rs is a remaining caller and \
         must not be broken by this lane"
    );
}

/// P2. The call site keeps the marker it upserts on.
#[test]
fn false_red_prevention_upsert_call_site_keeps_the_existing_marker() {
    let src = include_str!("../src/webhook/pipelines/review.rs");
    assert!(
        src.contains("\"<!-- ANVIL_SCORECARD_RECEIPT -->\""),
        "Expected False Red prevention: the upsert marker must be unchanged so \
         existing comments are amended, not duplicated"
    );
    let window = scorecard_upsert_call(src).join("\n");
    assert!(
        window.contains("upsert_pr_comment("),
        "Expected False Red prevention: the marker must still be the key the \
         scorecard is upserted on:\n{window}"
    );
}

// ---------------------------------------------------------------------------
// ABSENT EVIDENCE (I1).
// ---------------------------------------------------------------------------

/// P6. A gate that never ran must not be accused of failing. Missing tool.
#[test]
fn absent_evidence_a_missing_tool_renders_as_not_measured_never_failed() {
    let mut r = all_passing();
    r.cosign_status = GateStatus::NotMeasured {
        gate_id: "cosign_status".into(),
        reason: "cosign binary not found on PATH".into(),
    };
    r.recompute_unmeasured();
    r.summary_markdown = matrix_for(&r);

    let published = review::scorecard_comment(&r);
    assert!(
        published.contains("- **cosign** — not measured: cosign binary not found on PATH"),
        "absent evidence must be published as not measured:\n{published}"
    );
    assert!(
        !published.contains("- **cosign** — failed"),
        "must not fabricate an accusation against a gate that never ran:\n{published}"
    );
}

/// P5. A subprocess timeout is an error, not a pass, and the reason is carried
/// through verbatim so the reader can act on it.
#[test]
fn absent_evidence_a_subprocess_timeout_renders_as_errored_and_blocks() {
    let mut r = all_passing();
    r.test_suite_status =
        GateStatus::Errored("cargo test timed out after 1800s (build class)".into());
    seal(&mut r);

    let published = review::scorecard_comment(&r);
    assert!(
        published.contains("❌ Blocked"),
        "a timeout must block:\n{published}"
    );
    assert!(
        published.contains("errored: cargo test timed out after 1800s"),
        "the timeout reason must survive to the reader:\n{published}"
    );
}

/// P5. Unparseable tool output is an error, and several gates erroring at once
/// must all be reported rather than collapsed into one.
#[test]
fn absent_evidence_multiple_unparseable_outputs_are_all_reported() {
    let mut r = all_passing();
    r.coverage_status = GateStatus::Errored("llvm-cov emitted no parseable JSON".into());
    r.mutation_status = GateStatus::Errored("mutant report was truncated".into());
    seal(&mut r);

    let published = review::scorecard_comment(&r);
    assert!(
        published.contains("2 finding(s)"),
        "both errored gates must be counted:\n{published}"
    );
    assert!(
        published.contains("- **coverage** — errored:")
            && published.contains("- **mutation** — errored:"),
        "both errored gates must be named:\n{published}"
    );
}

// ---------------------------------------------------------------------------
// BOUNDARIES: one below, exactly at, one above.
// ---------------------------------------------------------------------------

/// The certified/blocked boundary sits at zero findings.
#[test]
fn boundary_zero_findings_certifies_and_one_finding_blocks() {
    // Exactly at: zero findings.
    let zero = review::scorecard_comment(&all_passing());
    assert!(zero.contains("✅ Certified"), "{zero}");

    // One above: a single finding.
    let mut one = all_passing();
    one.adr_status = GateStatus::Failed("ADR is missing the overturn_when field".into());
    seal(&mut one);
    let one_body = review::scorecard_comment(&one);
    assert!(one_body.contains("❌ Blocked — 1 finding(s)"), "{one_body}");

    // Two: the plural path must not double-count or drop.
    let mut two = all_passing();
    two.adr_status = GateStatus::Failed("missing overturn_when".into());
    two.doc_parity_status = GateStatus::Failed("3 docs reference a removed route".into());
    seal(&mut two);
    let two_body = review::scorecard_comment(&two);
    assert!(two_body.contains("❌ Blocked — 2 finding(s)"), "{two_body}");
    assert_eq!(two_body.matches("— failed:").count(), 2, "{two_body}");
}

/// The unmeasured boundary: zero, one, and two unmeasured gates.
#[test]
fn boundary_unmeasured_gate_count_is_reported_exactly() {
    let zero = review::scorecard_comment(&all_passing());
    assert!(
        !zero.contains("produced no measurement"),
        "zero unmeasured gates must say nothing:\n{zero}"
    );

    let mut one = all_passing();
    one.kani_status = GateStatus::NotMeasured {
        gate_id: "kani_status".into(),
        reason: "kani not installed".into(),
    };
    one.recompute_unmeasured();
    let one_body = review::scorecard_comment(&one);
    assert!(
        one_body.contains("1 gate(s) produced no measurement"),
        "{one_body}"
    );

    let mut two = all_passing();
    two.kani_status = GateStatus::NotMeasured {
        gate_id: "kani_status".into(),
        reason: "kani not installed".into(),
    };
    two.cosign_status = GateStatus::NotMeasured {
        gate_id: "cosign_status".into(),
        reason: "cosign not installed".into(),
    };
    two.recompute_unmeasured();
    let two_body = review::scorecard_comment(&two);
    assert!(
        two_body.contains("2 gate(s) produced no measurement"),
        "{two_body}"
    );
}

/// P12. The worst case -- every gate failing -- must still fit in a GitHub
/// comment, and must still be smaller than the table it replaces.
#[test]
fn boundary_worst_case_body_fits_a_github_comment_and_beats_the_table() {
    let mut r = all_passing();
    macro_rules! fail_all {
        ($($f:ident),* $(,)?) => { $( r.$f = GateStatus::Failed("seeded failure".into()); )* };
    }
    fail_all!(
        doc_parity_status,
        cedar_status,
        compliance_status,
        api_contract_status,
        cell_isolation_status,
        supply_chain_status,
        clean_arch_status,
        monorepo_status,
        debt_shrink_status,
        modularization_status,
        coverage_status,
        rust_skills_status,
        kani_status,
        slo_status,
        adr_status,
        shuffle_status,
        trace_status,
        constant_work_status,
        idempotency_status,
        finops_status,
        ghost_migration_status,
        gitops_promo_status,
        gitops_drift_status,
        canary_status,
        cluster_audit_status,
        migration_orch_status,
        ci_wallclock_status,
        predictive_test_status,
        compile_profile_status,
        remote_cache_status,
        runner_economics_status,
        sandbox_status,
        cross_service_status,
        ephemeral_secret_status,
        psa_status,
        shadow_traffic_status,
        unresolved_review_status,
        local_probe_status,
        semantic_abi_status,
        zero_day_status,
        formal_verification_status,
        deadlock_status,
        automated_canary_status,
        progressive_ring_status,
        hermetic_build_status,
        openvex_status,
        cosign_status,
        chaos_injection_status,
        stacked_diffs_status,
        microbench_status,
        jittered_backoff_status,
        schema_evolution_status,
        auto_rollback_status,
        wasm_sandbox_status,
        consistency_status,
        flake_quarantine_status,
        zero_trust_workload_status,
        carbon_compute_status,
        replay_harness_status,
        upgrade_train_status,
        mutation_status,
        feature_flag_status,
        bench_status,
        attestation_status,
        security_scan_status,
        schema_compat_status,
        performance_concurrency_status,
        test_suite_status
    );
    seal(&mut r);

    let published = review::scorecard_comment(&r);
    assert!(
        published.len() < GITHUB_COMMENT_LIMIT,
        "worst-case body is {} chars, above GitHub's {} limit",
        published.len(),
        GITHUB_COMMENT_LIMIT
    );
    assert!(
        published.contains("68 finding(s)"),
        "every failing gate must be reported:\n{}",
        &published[..published.len().min(400)]
    );
    // "beats the table" was in the name but unasserted: even in the worst case,
    // where the terse form has the least to gain, it must still be smaller than
    // the matrix it replaces.
    let matrix = matrix_for(&r);
    assert!(
        published.len() < matrix.len(),
        "worst-case terse body is {} chars against the matrix's {}; the \
         rendering must not be larger than what it replaces",
        published.len(),
        matrix.len()
    );
}

/// P7  The fidelity registry records that ~21 gates are heuristic or
///     aspirational, and `finding_line` attaches that note to the scorecard.
///     But `finding_line` runs only for Failed / Errored / NotMeasured /
///     Warning gates. A gate that PASSES produces no line, so it carries no
///     note -- and the certified branch discards `findings` entirely and
///     publishes one sentence: "Certified — N/N gates passed."
///
///     So the disclosure renders only on the failure path, which is exactly
///     when it is least needed. On the green path -- the only moment a human
///     decides whether to trust the certification -- a reader sees a full
///     score and nothing at all about how much of it is a keyword scan.
#[test]
fn a_certified_scorecard_discloses_how_many_passing_gates_are_low_fidelity() {
    let body = publish::scorecard::render(&all_passing());

    assert!(
        body.contains("do not fully measure"),
        "a certified scorecard must disclose that some passing gates are \
         heuristic or partial; it published:\n{body}"
    );

    for gate in ["kani", "mutation", "zero-trust-workload"] {
        assert!(
            body.contains(gate),
            "gate {gate} is registry-recorded as heuristic or partial and \
             passed, so it must be named in the disclosure:\n{body}"
        );
    }

    // The disclosure has to discriminate. Naming every gate would satisfy the
    // assertions above while telling a reader nothing -- so a gate the
    // registry does not record as low fidelity must be absent from the list.
    let disclosure = body
        .split_once("do not fully measure")
        .expect("disclosure line")
        .1;
    // `debt-shrink` used to stand here as a gate the registry did not record.
    // It does now: this pull request audited it and entered it as Heuristic,
    // so naming it in the disclosure became correct and it is no longer a
    // negative example. `idempotency` replaces it -- still unaudited, so the
    // list keeps exactly as many gates that must NOT appear.
    for gate in ["cell-isolation", "monorepo", "idempotency"] {
        assert!(
            !disclosure.contains(gate),
            "gate {gate} is not recorded below Measured fidelity, so naming it \
             in the disclosure makes the list meaningless:\n{body}"
        );
    }

    // `cosign_status` is `Fidelity::Aspirational`. A real certification run
    // withholds its pass before sealing, so it is disclosed as a gate that
    // produced no measurement. Naming it here as well would put the same gate
    // on one scorecard as both "passed, but does not fully measure" and
    // "produced no measurement". This fixture reaches the green branch only
    // because it is built through `from_gate_outcomes`, which does not apply
    // the ceiling -- which is exactly why the exclusion has to be in the
    // renderer and pinned here.
    assert!(
        !disclosure.contains("cosign"),
        "cosign is registry-recorded as aspirational, so it cannot be \
         disclosed as a passing gate that measures imperfectly:\n{body}"
    );
}
