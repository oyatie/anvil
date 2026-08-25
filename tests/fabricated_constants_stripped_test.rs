//! Lane `strip-fabricated-constants`: the gates that fabricate their own inputs
//! and are therefore unfailable by arithmetic.
//!
//! Five when this file was written. Three more were found later --
//! `canary_status`, `shuffle_status` and `progressive_ring_status` -- and the
//! way they were missed matters more than the fact of them: every mechanism
//! below was already general enough to catch all three, and every one of them
//! was pointed at a hardcoded list of five module directories. A ratchet is only
//! as wide as its scan. Sections 6, 7 and 8 add those gates, and the scan lists
//! now carry all eight.
//!
//! # Premortem
//!
//! Assume this change has already failed in production. The ways it can have
//! failed, each turned into a test below:
//!
//! P1. The constants were deleted from the *report* but the gate still returns
//!     `Passed`, so the scorecard is unchanged and nothing was actually fixed.
//!     -> `*_reports_not_measured` (red->green).
//! P2. The constants were "moved, not removed" -- pushed down into a helper, a
//!     `const`, a sibling file in the same module directory, a `Default` impl
//!     or a test fixture that production code still reads. Or kept in place and
//!     merely re-valued (`142` -> `137`), which defeats any list of literals.
//!     The gate looks honest and behaves identically.
//!     -> `*_constants_are_absent_from_source` scans every `.rs` file in the
//!        gate's module directory, and
//!        `owned_gate_callers_assign_no_numeric_literal_measurements` bans
//!        assigned numeric literals outright in the five callers. Both are
//!        mechanisms over source text (I22) -- prose in a comment cannot
//!        enforce this.
//! P3. Over-correction: with no data source the gate reports `Failed`, so every
//!     PR in the fleet is accused of an SLO breach / cache regression / cluster
//!     drift that nobody can reproduce. I1 cuts both ways: absent evidence is
//!     not a pass AND not an accusation.
//!     -> `*_false_red_prevention_*` and `no_owned_gate_fabricates_an_accusation`.
//! P4. The real measurement that *did* exist gets deleted along with the fake
//!     one. `slo_canary_guard` genuinely parses OpenSLO YAML off disk, and
//!     `CacheHitRateRatchet` and `TrafficMirrorComparator` genuinely compute
//!     over caller-supplied
//!     metrics; only their *callers* fabricate. If those go too, wiring a real
//!     data source in stage 3 has nothing to wire into.
//!     -> `*_still_*` boundary tests, each pinned at / one below / one above.
//!     `ClusterDiffEvaluator` is the exception and is NOT a surviving tool: it
//!     fires only on `live.contains("replicas: 10") && git.contains("replicas: 3")`,
//!     so it is a sixth fabricated constant sitting one file from the caller.
//!     Its boundary test asserts drift on operands it was not written against,
//!     and is red for that reason.
//! P5. The gate reports `NotMeasured` with a `gate_id` that does not match the
//!     field name on `PreMergeCertificationReport`, so `unmeasured_gates` names
//!     a gate nobody can find and merge admission silently fails to block.
//!     -> `unmeasured_gate_ids_match_the_fidelity_registry`.
//! P6. The summary string keeps asserting the number that was deleted ("✅ PASSED
//!     (Error budget burn rate 1h: 1.02x ...)"). The struct is honest, the
//!     published comment is not -- and the comment is what a human reads.
//!     -> `*_summary_*` assertions.
//! P7. Absent evidence is swallowed rather than reported: a spec named in the
//!     diff that is missing or unreadable on disk is skipped by an `if let Ok`,
//!     leaving `violations` empty and the gate green.
//!     -> `*_absent_evidence_*`.
//!
//! Naming matches `tests/red_green_gates_test.rs` ("False Green prevention" /
//! "False Red prevention") so the two files read as one suite.

// (no blanket allow: an import that stops being exercised is a test that stopped testing)

use anvil::canary_rollout::{CanaryMetricsSnapshot, CanaryRolloutGuard};
use anvil::ci_wallclock_ratchet::CiWallclockEconomicsRatchet;
use anvil::cluster_state_auditor::{ClusterDiffEvaluator, ClusterStateAuditor};
use anvil::git_manager::PrDiffContext;
use anvil::pre_merge_guard::GateStatus;
use anvil::progressive_rollout::{
    DeploymentRing, ProgressiveRingOrchestrator, RingConfig, RolloutManifest,
};
use anvil::remote_cache_optimizer::{CacheHitMetrics, CacheHitRateRatchet, RemoteCacheOptimizer};
use anvil::shadow_traffic_harness::{
    ShadowTrafficHarness, ShadowTrafficMetrics, TrafficMirrorComparator,
};
use anvil::shuffle_shard_simulator::{ShuffleShardAllocation, ShuffleShardSimulator};
use anvil::slo_canary_guard::SloCanaryGuard;
use std::path::Path;

// -------------------------------------------------------------------------
// Fixtures
// -------------------------------------------------------------------------

fn diff_ctx(files: &[&str], diff_content: &str, working_dir: &Path) -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/console".to_string(),
        pr_number: 4242,
        base_branch: "main".to_string(),
        base_sha: "base123".to_string(),
        head_sha: "head456".to_string(),
        previous_head_sha: None,
        repo_working_dir: working_dir.to_path_buf(),
        diff_content: diff_content.to_string(),
        changed_files: files.iter().map(|f| f.to_string()).collect(),
        is_incremental: false,
    }
}

/// A PR that touches ordinary code: no SLO spec, no infra manifest, nothing
/// that could legitimately produce a finding from any of the five gates.
fn clean_diff(working_dir: &Path) -> PrDiffContext {
    diff_ctx(
        &["src/handler.rs"],
        "+ pub fn healthz() -> &'static str { \"ok\" }",
        working_dir,
    )
}

/// The production half of a source file: everything before `#[cfg(test)]`.
///
/// Fixture constants inside a test module are legitimate -- they are inputs a
/// test supplies, which is exactly what a real data source will supply later.
/// A constant in the production half is the defect.
fn production_source(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    match s.find("#[cfg(test)]") {
        Some(i) => s[..i].to_string(),
        None => s,
    }
}

/// Every `.rs` file under a gate's module directory, as (repo-relative path,
/// production half).
///
/// Scanning only `mod.rs` would leave the cheapest evasion of P2 wide open:
/// each of these five gates already owns sibling files (`diff_evaluator.rs`,
/// `regression_budget.rs`, `traffic_mirror.rs`, ...), so "moved, not removed"
/// most plausibly means moved one file sideways into a `fn default_snapshot()`
/// the caller still reads. The scan follows the constant (I22).
fn module_production_sources(module_dir: &str) -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join(module_dir);
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
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
                out.push((rel.clone(), production_source(&rel)));
            }
        }
    }
    assert!(!out.is_empty(), "no sources found under {module_dir}");
    out.sort();
    out
}

/// Asserts no file in a gate's module directory carries any banned literal in
/// its production half.
fn assert_absent_from_module(module_dir: &str, needles: &[&str]) {
    for (rel, src) in module_production_sources(module_dir) {
        let found: Vec<&str> = needles
            .iter()
            .copied()
            .filter(|n| src.contains(n))
            .collect();
        assert!(
            found.is_empty(),
            "False Green prevention: {rel} still contains fabricated measurement constant(s) \
             {found:?} in production code. A gate whose input is a literal is indistinguishable \
             from a hardcoded constant (I2)."
        );
    }
}

/// Blanks out the contents of double-quoted strings and drops `//` comments,
/// so a scan for numeric literals sees code only. Each blanked character is
/// replaced one-for-one, so column positions still line up with the file.
///
use anvil::source_scan::code_only;

/// Numeric literals *assigned* in the production half of a gate's caller.
///
/// This is the generalisation of the fixed needle lists: those catch the six
/// constants named in the brief, this catches the seventh. Substituting `142`
/// with `137`, or `5000` with `4711`, defeats a needle list and does not defeat
/// this (I22).
///
/// Deliberately narrow, so it cannot become the reason a real implementation is
/// blocked and the check bypassed (P3):
///   - string and comment contents are excluded, so a `reason:` naming an
///     endpoint or a version cannot trip it;
///   - comparisons (`>=`, `<=`, `==`, `!=`) are excluded: a threshold compared
///     against a *measured* value is legitimate, a fabricated input is not;
///   - `= 0` and `= 1` are excluded: initialising a counter is not a measurement.
///   - a `const` or `static` declaration whose NAME says it is a bound is
///     excluded: `MIN_ACCEPTABLE_CACHE_HIT_RATE_PCT` is the same thing as the
///     `3.0` ceiling a caller passes in, and the exemption for comparisons
///     above already says a bound is legitimate. Firing on it would make this
///     rule fire on honest code, which is how a rule gets deleted rather than
///     satisfied. What is NOT exempt is the value flowing anywhere else: into a
///     `let`, into a struct field, into an argument. A reading arrives; a bound
///     is declared.
///
///     The exemption is keyed on the name and not on the `const` keyword,
///     because "move the reading into a `const` and read it back" is otherwise
///     wide open: `pub const OBSERVED_P99_LATENCY_MS: f64 = 137.5;` in
///     `slo_canary_guard` is a fabricated reading wearing a keyword, and a
///     keyword-only exemption cannot see it. `no_owned_gate_reports_passed_without_a_data_source`
///     is the behavioural backstop, but it only fires once a gate reaches
///     `Passed`; a gate that abstains while carrying a fabricated number in its
///     report is exactly the shape this scan exists to catch, and is invisible
///     to it. So: `MIN_`/`MAX_`/`_THRESHOLD`/`_CEILING`/`_LIMIT`/`_BUDGET` name
///     a bound and pass, and every other `const` is scanned like any other line.
fn assigned_numeric_literals(rel: &str) -> Vec<String> {
    /// A `const` naming one of these is declaring a bound, not recording a
    /// reading. Anything else is scanned.
    const BOUND_NAME_MARKERS: [&str; 6] = [
        "MIN_",
        "MAX_",
        "_THRESHOLD",
        "_CEILING",
        "_LIMIT",
        "_BUDGET",
    ];

    let mut hits = Vec::new();
    for line in production_source(rel).lines() {
        let code: Vec<char> = code_only(line).chars().collect();
        let declared_bound = {
            let t: String = code.iter().collect();
            let t = t.trim().trim_start_matches("pub ").trim_start();
            (t.starts_with("const ") || t.starts_with("static "))
                && BOUND_NAME_MARKERS.iter().any(|m| t.contains(m))
        };
        if declared_bound {
            continue;
        }
        for i in 0..code.len() {
            if !code[i].is_ascii_digit() {
                continue;
            }
            // Only the first character of a literal, and never an identifier
            // such as `burn_rate_1h` or a field of `self.0`.
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
            if prev != ':' && prev != '=' {
                continue;
            }
            // `>=`, `<=`, `==`, `!=` are comparisons, not assignments.
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
            hits.push(format!("`{}` in `{}`", lit, line.trim()));
            break;
        }
    }
    hits
}

fn assert_not_measured(status: &GateStatus, expect_gate_id: &str, reason_must_name: &[&str]) {
    match status {
        GateStatus::NotMeasured { gate_id, reason } => {
            assert_eq!(
                gate_id, expect_gate_id,
                "gate_id must match the PreMergeCertificationReport field name so \
                 unmeasured_gates names a gate a human can find"
            );
            let lower = reason.to_lowercase();
            assert!(
                reason_must_name
                    .iter()
                    .any(|n| lower.contains(&n.to_lowercase())),
                "reason must name the missing data source (one of {reason_must_name:?}); got: {reason}"
            );
        }
        other => panic!(
            "I1: with no data source the gate must report NotMeasured, got {other:?}. \
             Passed would make absent evidence a pass; Failed would fabricate an accusation."
        ),
    }
}

fn assert_no_accusation(status: &GateStatus) {
    assert!(
        !matches!(status, GateStatus::Failed(_) | GateStatus::Errored(_)),
        "False Red prevention: a gate with no data source must not accuse a clean PR; got {status:?}"
    );
}

// =========================================================================
// 1. SloCanaryGuard -- gate_id `slo_status`
//    Fabricated: simulated_burn_rate_1h = 1.02 vs a 14.4 threshold.
//    Missing data source: a reachable Prometheus / OpenTelemetry endpoint.
// =========================================================================

#[test]
fn test_slo_canary_reports_not_measured_without_a_telemetry_endpoint() {
    // RED->GREEN. No burn rate was ever queried, so the gate has nothing to say.
    let tmp = tempfile::tempdir().expect("tempdir");
    let guard = SloCanaryGuard::new();
    let rep = guard
        .evaluate_slo_canary_health(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    assert_not_measured(
        &rep.status,
        "slo_status",
        &["prometheus", "opentelemetry", "telemetry endpoint"],
    );
}

#[test]
fn test_slo_canary_absent_evidence_no_burn_rate_or_pass_is_published() {
    // P6, the assertion the other four gates already carry and this one did
    // not: the summary is what a human reads on the PR, so an honest `status`
    // behind a string that still announces a healthy burn rate changes nothing.
    // Deleting the *numbers* is not enough -- "✅ PASSED (burn rate nominal)"
    // asserts a measurement in prose (I2).
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = SloCanaryGuard::new()
        .evaluate_slo_canary_health(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    let lower = rep.summary.to_lowercase();
    assert!(
        !lower.contains("burn rate"),
        "I2: no burn rate was queried, so none may be described; got {}",
        rep.summary
    );
    assert!(
        !lower.contains("passed"),
        "I1: an unmeasured gate must not announce a pass; got {}",
        rep.summary
    );
    assert!(
        rep.violations.is_empty(),
        "False Red prevention: a PR with no SLO spec has no SLO defect; got {:?}",
        rep.violations
    );
}

#[test]
fn test_slo_canary_false_green_prevention_burn_rate_constants_are_absent_from_source() {
    // P2: the constant must be gone, not relocated. Enforced over the source
    // text because no comment can stop the next author reintroducing it (I22).
    assert_absent_from_module(
        "src/slo_canary_guard",
        &[
            "simulated_burn_rate",
            "1.02",
            "1.01",
            "14.4",
            "max_burn_rate",
            "Nominal healthy burn rate",
        ],
    );
}

#[test]
fn test_slo_canary_false_green_prevention_zero_objective_spec_is_still_rejected() {
    // P4: the OpenSLO parse is a REAL measurement over a real file. It must
    // survive the removal of the fake burn rate, and its finding must not be
    // described in terms of a burn rate that was never measured (P6).
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("service.openslo.yaml"),
        "apiVersion: openslo/v1\nkind: SLO\nmetadata:\n  name: api\nspec:\n  service: console\n  objectives: []\n",
    )
    .unwrap();

    let guard = SloCanaryGuard::new();
    let rep = guard
        .evaluate_slo_canary_health(
            tmp.path(),
            &diff_ctx(&["service.openslo.yaml"], "+ objectives: []", tmp.path()),
        )
        .expect("gate runs");

    assert!(
        matches!(rep.status, GateStatus::Failed(_)),
        "False Green prevention: an OpenSLO spec declaring 0 objectives is a measured \
         defect and must FAIL, got {:?}",
        rep.status
    );
    assert_eq!(rep.slos_evaluated, 1, "the spec was read, so count it");
    let lower = rep.summary.to_lowercase();
    assert!(
        !lower.contains("burn"),
        "P6: the finding is a spec defect; describing it as a burn-rate violation \
         reports a measurement that was never taken (I2). Summary: {}",
        rep.summary
    );
}

#[test]
fn test_slo_canary_false_red_prevention_valid_spec_is_not_accused() {
    // P3: a well-formed spec plus no telemetry is NOT a violation. The gate
    // measured the half it can (the spec) and must report the half it cannot
    // as absent rather than as a breach.
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("service.openslo.yaml"),
        "apiVersion: openslo/v1\nkind: SLO\nmetadata:\n  name: api\nspec:\n  service: console\n  objectives:\n    - displayName: 99.9%\n      target: 0.999\n",
    )
    .unwrap();

    let guard = SloCanaryGuard::new();
    let rep = guard
        .evaluate_slo_canary_health(
            tmp.path(),
            &diff_ctx(&["service.openslo.yaml"], "+ target: 0.999", tmp.path()),
        )
        .expect("gate runs");

    assert_no_accusation(&rep.status);
    assert!(
        rep.violations.is_empty(),
        "False Red prevention: a valid OpenSLO spec must produce no violation; got {:?}",
        rep.violations
    );
    assert_not_measured(
        &rep.status,
        "slo_status",
        &["prometheus", "opentelemetry", "telemetry endpoint"],
    );
}

#[test]
fn test_slo_canary_absent_evidence_spec_named_in_diff_but_missing_on_disk() {
    // P7. Today `if full_path.exists()` skips silently and the gate goes green:
    // a spec it could not read is scored as a spec with no problems.
    let tmp = tempfile::tempdir().expect("tempdir");
    let guard = SloCanaryGuard::new();
    let rep = guard
        .evaluate_slo_canary_health(
            tmp.path(),
            &diff_ctx(&["gone.openslo.yaml"], "+ objectives: []", tmp.path()),
        )
        .expect("gate runs");

    assert!(
        !matches!(rep.status, GateStatus::Passed),
        "I1: a spec named in the diff that could not be read is absent evidence, \
         never a pass; got {:?}",
        rep.status
    );
    assert_eq!(
        rep.slos_evaluated, 0,
        "nothing was parsed, so nothing may be counted as evaluated (I2)"
    );
}

#[test]
fn test_slo_canary_absent_evidence_unparseable_spec_is_errored() {
    // A file that exists but is not OpenSLO YAML: a parse failure, which I1
    // routes to Errored -- not to a Failed that names a defect in the PR's SLO
    // policy, and not to a silent skip.
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("broken.openslo.yaml"),
        "this: [is: not: valid: yaml\n  - {{{\n",
    )
    .unwrap();

    let guard = SloCanaryGuard::new();
    let rep = guard
        .evaluate_slo_canary_health(
            tmp.path(),
            &diff_ctx(&["broken.openslo.yaml"], "+ {{{", tmp.path()),
        )
        .expect("gate runs");

    assert!(
        matches!(rep.status, GateStatus::Errored(_)),
        "I1: an unparseable spec is a failure to measure, not a measured failure; got {:?}",
        rep.status
    );
}

#[test]
fn test_slo_canary_boundary_objective_target_at_below_and_above_one() {
    // Boundary at the one threshold that is genuinely measured: 0.0 < target <= 1.0.
    // Both bounds, each at / one inside / one outside:
    //   lower: 0.0 is outside (exclusive), 0.0000001 is the first value inside;
    //   upper: 1.0 is inside (inclusive), 1.0000001 is the first value outside.
    let cases: [(&str, bool); 4] = [
        ("0.0", false),
        ("0.0000001", true),
        ("1.0", true),
        ("1.0000001", false),
    ];
    for (target, expect_valid) in cases {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            tmp.path().join("b.openslo.yaml"),
            format!(
                "apiVersion: openslo/v1\nkind: SLO\nmetadata:\n  name: api\nspec:\n  service: console\n  objectives:\n    - target: {target}\n"
            ),
        )
        .unwrap();

        let rep = SloCanaryGuard::new()
            .evaluate_slo_canary_health(
                tmp.path(),
                &diff_ctx(&["b.openslo.yaml"], "+ target", tmp.path()),
            )
            .expect("gate runs");

        if expect_valid {
            assert!(
                rep.violations.is_empty(),
                "target {target} is exactly at the bound and must be accepted; got {:?}",
                rep.violations
            );
            assert_not_measured(
                &rep.status,
                "slo_status",
                &["prometheus", "opentelemetry", "telemetry endpoint"],
            );
        } else {
            assert!(
                matches!(rep.status, GateStatus::Failed(_)),
                "target {target} is out of bounds and must FAIL; got {:?}",
                rep.status
            );
        }
    }
}

// =========================================================================
// 2. RemoteCacheOptimizer -- gate_id `remote_cache_status`
//    Fabricated: hit_rate_pct 95.0 vs an 85.0 threshold, over a
//    "Cargo.lock.mock" that does not exist.
//    Missing data source: sccache / Buck2 CAS statistics.
// =========================================================================

#[test]
fn test_remote_cache_reports_not_measured_without_cas_statistics() {
    // RED->GREEN.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = RemoteCacheOptimizer::new()
        .evaluate_cache_alignment(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    assert_not_measured(
        &rep.status,
        "remote_cache_status",
        &["sccache", "buck2", "cas statistics"],
    );
}

#[test]
fn test_remote_cache_false_green_prevention_hit_rate_constants_are_absent_from_source() {
    assert_absent_from_module(
        "src/remote_cache_optimizer",
        &[
            "hit_rate_pct: 95.0",
            "cache_hits: 114",
            "cache_misses: 6",
            "total_compilation_units: 120",
            "sample_metrics",
            "Cargo.lock.mock",
            "rustc-1.85.0-nightly",
        ],
    );
}

#[test]
fn test_remote_cache_absent_evidence_no_cache_key_is_reported_for_an_absent_lockfile() {
    // P6/P7: the published summary today carries a cache key computed from the
    // literal string "Cargo.lock.mock" -- a key for a lockfile that was never
    // read. A key over absent input is a fabricated identifier, not a measurement.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = RemoteCacheOptimizer::new()
        .evaluate_cache_alignment(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    assert!(
        !rep.summary.contains("sccache-v2-"),
        "I2: no lockfile was read, so no cache key may be published; got {}",
        rep.summary
    );
    assert!(
        !rep.summary.contains("95.0")
            && !rep.summary.to_lowercase().contains("hit rate is optimal"),
        "I2: no hit rate was measured, so none may be published; got {}",
        rep.summary
    );
}

#[test]
fn test_remote_cache_false_red_prevention_ratchet_still_fails_a_cold_cache_at_the_boundary() {
    // P4: the ratchet is a legitimate pure function over caller-supplied
    // metrics and must survive. Boundary: exactly at 85.0, one below, one above.
    let ratchet = CacheHitRateRatchet::new();
    let at = |pct: f64| {
        ratchet
            .evaluate_cache_efficiency(&CacheHitMetrics {
                total_compilation_units: 100,
                cache_hits: 0,
                cache_misses: 0,
                hit_rate_pct: pct,
            })
            .is_optimal
    };
    assert_eq!(CacheHitRateRatchet::MIN_ACCEPTABLE_CACHE_HIT_RATE_PCT, 85.0);
    assert!(at(85.0), "exactly at the threshold must pass");
    assert!(!at(84.9), "one below the threshold must FAIL");
    assert!(at(85.1), "one above the threshold must pass");
}

// =========================================================================
// 3. CiWallclockEconomicsRatchet -- gate_id `ci_wallclock_status`
//    Fabricated: pr_wallclock_seconds 142, comment "Under 5 min ceiling!".
//    Missing data source: the GitHub Actions timing API.
// =========================================================================

#[test]
fn test_ci_wallclock_reports_not_measured_without_the_actions_timing_api() {
    // RED->GREEN.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = CiWallclockEconomicsRatchet::new()
        .evaluate_ci_efficiency(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    assert_not_measured(
        &rep.status,
        "ci_wallclock_status",
        &["github actions", "timing api", "workflow run timing"],
    );
}

#[test]
fn test_ci_wallclock_false_green_prevention_wallclock_constants_are_absent_from_source() {
    assert_absent_from_module(
        "src/ci_wallclock_ratchet",
        &[
            "pr_wallclock_seconds: 142",
            "trunk_baseline_seconds: 150",
            "billable_compute_cost_usd: 0.045",
            "trunk_baseline_cost_usd: 0.050",
            "Under 5 min ceiling",
        ],
    );
}

#[test]
fn test_ci_wallclock_absent_evidence_no_seconds_or_dollars_are_published() {
    // P6: the summary today reads "PR GHA wallclock: 142s ... compute cost:
    // $0.045". Both numbers are literals. A reader treats them as measurements.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = CiWallclockEconomicsRatchet::new()
        .evaluate_ci_efficiency(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    assert!(
        !rep.summary.contains("142"),
        "I2: no wallclock was measured, so no second-count may be published; got {}",
        rep.summary
    );
    assert!(
        !rep.summary.contains('$'),
        "I2: no billing data was read, so no cost may be published; got {}",
        rep.summary
    );
    assert!(
        !rep.summary.contains("PASSED"),
        "I1: an unmeasured gate must not announce a pass; got {}",
        rep.summary
    );
}

// =========================================================================
// 4. ClusterStateAuditor -- gate_id `cluster_audit_status`
//    Fabricated: two IDENTICAL hardcoded literals, "replicas: 3" vs "replicas: 3".
//    Missing data source: Kubernetes API / ArgoCD access.
// =========================================================================

#[test]
fn test_cluster_audit_reports_not_measured_without_cluster_access() {
    // RED->GREEN.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = ClusterStateAuditor::new()
        .evaluate_cluster_parity(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    assert_not_measured(
        &rep.status,
        "cluster_audit_status",
        &["kubernetes", "argocd", "cluster access"],
    );
}

#[test]
fn test_cluster_audit_false_green_prevention_identical_literals_are_absent_from_source() {
    // The purest form of the defect: a comparison whose two operands are the
    // same literal. It can only ever return "synchronized".
    assert_absent_from_module(
        "src/cluster_state_auditor",
        // `live_manifest` / `git_manifest` are banned only in their *assigned a
        // string literal* form: they are legitimate parameter names for a real
        // readback, and banning the identifier outright would block the
        // implementation this lane is clearing the way for (P3).
        &["replicas: 3", "live_manifest = \"", "git_manifest = \""],
    );
}

#[test]
fn test_cluster_audit_absent_evidence_never_claims_synchronization() {
    // P6: "✅ PASSED (Live cluster state is 100% synchronized with Git
    // declarative desired-state)" is published today without contacting a
    // cluster. That sentence is the whole lie in one line.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = ClusterStateAuditor::new()
        .evaluate_cluster_parity(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    let lower = rep.summary.to_lowercase();
    assert!(
        !lower.contains("synchronized") && !lower.contains("passed"),
        "I1/I2: no cluster was read, so parity may not be claimed; got {}",
        rep.summary
    );
    assert!(
        rep.drift_findings.is_empty(),
        "False Red prevention: no cluster was read, so no drift may be alleged"
    );
}

#[test]
fn test_cluster_audit_false_red_prevention_diff_evaluator_still_detects_supplied_drift() {
    // P4: the evaluator is the seam a real readback will plug into, so it must
    // survive. But it only counts as a seam if it discriminates on the operands
    // it is *given*.
    //
    // REVIEW FINDING (this assertion is why the test is red): today
    // `compare_cluster_state` fires on exactly one pair of literals --
    // `live.contains("replicas: 10") && git.contains("replicas: 3")` -- which is
    // the sixth fabricated constant in this lane, one file sideways from the
    // caller in `diff_evaluator.rs`. Asserting only that magic pair, as this
    // test originally did, passes against a comparison that can never report
    // anything else: the same unfailable shape the lane exists to delete, and
    // the reason `git` vs `git` returning empty proves nothing (every input but
    // the magic one returns empty).
    let eval = ClusterDiffEvaluator::new();
    let manifest = |replicas: &str, image: &str| {
        format!(
            "apiVersion: apps/v1\nkind: Deployment\nmetadata:\n  name: console\nspec:\n  replicas: {replicas}\n  template:\n    spec:\n      containers:\n        - image: {image}\n"
        )
    };

    // Values the implementation was written against.
    assert_eq!(
        eval.compare_cluster_state(
            &manifest("10", "console:1.4.0"),
            &manifest("3", "console:1.4.0")
        )
        .len(),
        1,
        "supplied drift must still be detected"
    );
    // The same drift, in values it was NOT written against. A comparison that
    // recognises only its own literals is a constant, not a measurement (I2).
    assert_eq!(
        eval.compare_cluster_state(
            &manifest("5", "console:1.4.0"),
            &manifest("2", "console:1.4.0")
        )
        .len(),
        1,
        "drift in a replica count the evaluator was not hardcoded for must be \
         detected too, or the evaluator is itself a fabricated constant"
    );
    // Drift in a different field entirely: the readback will not always differ
    // on replica counts.
    assert!(
        !eval
            .compare_cluster_state(
                &manifest("3", "console:1.4.1-hotfix"),
                &manifest("3", "console:1.4.0")
            )
            .is_empty(),
        "an out-of-band image change is drift; a differ that only knows about \
         replicas cannot audit a cluster"
    );
    // False Red prevention: identical manifests must produce no finding, so a
    // synchronised cluster is not accused.
    for m in [
        manifest("3", "console:1.4.0"),
        manifest("5", "console:1.4.1-hotfix"),
    ] {
        assert!(
            eval.compare_cluster_state(&m, &m).is_empty(),
            "identical manifests must produce no finding"
        );
    }
}

// =========================================================================
// 5. ShadowTrafficHarness -- gate_id `shadow_traffic_status`
//    Fabricated: sampled_requests 5000, payload_parity_pct 99.98.
//    Missing data source: traffic mirroring infrastructure and a replay target.
// =========================================================================

#[test]
fn test_shadow_traffic_reports_not_measured_without_a_mirror() {
    // RED->GREEN.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = ShadowTrafficHarness::new()
        .evaluate_shadow_verification(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    assert_not_measured(
        &rep.status,
        "shadow_traffic_status",
        &["traffic mirror", "mirroring", "replay target"],
    );
}

#[test]
fn test_shadow_traffic_false_green_prevention_parity_constants_are_absent_from_source() {
    assert_absent_from_module(
        "src/shadow_traffic_harness",
        &[
            "sampled_requests: 5000",
            "payload_parity_pct: 99.98",
            "status_code_parity_pct: 100.0",
            "latency_delta_pct: 0.8",
            "let baseline",
        ],
    );
}

#[test]
fn test_shadow_traffic_absent_evidence_no_request_count_or_parity_is_published() {
    // P6: "verified: 99.98% payload parity ... across 5000 sampled requests"
    // names a sample size for a sample that was never taken.
    let tmp = tempfile::tempdir().expect("tempdir");
    let rep = ShadowTrafficHarness::new()
        .evaluate_shadow_verification(tmp.path(), &clean_diff(tmp.path()))
        .expect("gate runs");
    assert!(
        !rep.summary.contains("5000") && !rep.summary.contains("99.98"),
        "I2: no requests were mirrored, so no sample size or parity may be published; got {}",
        rep.summary
    );
    assert!(
        !rep.summary.to_lowercase().contains("verified"),
        "I1: nothing was verified; got {}",
        rep.summary
    );
}

#[test]
fn test_shadow_traffic_false_red_prevention_comparator_still_fails_at_the_boundary() {
    // P4: boundaries on the two retained thresholds, 99.5% payload parity and
    // 99.9% status-code parity.
    let comp = TrafficMirrorComparator::new();
    let at = |payload: f64, status: f64| {
        comp.evaluate_shadow_parity(&ShadowTrafficMetrics {
            sampled_requests: 10,
            payload_parity_pct: payload,
            status_code_parity_pct: status,
            latency_delta_pct: 0.0,
        })
        .is_parity_satisfied
    };
    assert!(at(99.5, 99.9), "exactly at both thresholds must pass");
    assert!(!at(99.49, 99.9), "one below payload parity must FAIL");
    assert!(at(99.51, 99.9), "one above payload parity must pass");
    assert!(!at(99.5, 99.89), "one below status parity must FAIL");
}

// =========================================================================
// Cross-cutting: I1 in both directions, over all five gates at once.
// =========================================================================

/// Every owned gate, run against the same clean fixture, as (name, status).
fn all_owned_statuses(tmp: &Path) -> Vec<(&'static str, GateStatus)> {
    let d = clean_diff(tmp);
    vec![
        (
            "slo_status",
            SloCanaryGuard::new()
                .evaluate_slo_canary_health(tmp, &d)
                .expect("slo")
                .status,
        ),
        (
            "remote_cache_status",
            RemoteCacheOptimizer::new()
                .evaluate_cache_alignment(tmp, &d)
                .expect("cache")
                .status,
        ),
        (
            "ci_wallclock_status",
            CiWallclockEconomicsRatchet::new()
                .evaluate_ci_efficiency(tmp, &d)
                .expect("wallclock")
                .status,
        ),
        (
            "cluster_audit_status",
            ClusterStateAuditor::new()
                .evaluate_cluster_parity(tmp, &d)
                .expect("cluster")
                .status,
        ),
        (
            "shadow_traffic_status",
            ShadowTrafficHarness::new()
                .evaluate_shadow_verification(tmp, &d)
                .expect("shadow")
                .status,
        ),
        // The three gates added by the `unfailable-by-arithmetic` lane. None
        // takes the fixture: each one's abstaining constructor takes no
        // argument at all, which is the point -- there is nothing to hand it.
        (
            "canary_status",
            CanaryRolloutGuard::new()
                .evaluate_without_metrics_source()
                .status,
        ),
        (
            "shuffle_status",
            ShuffleShardSimulator::new()
                .evaluate_without_topology_source()
                .status,
        ),
        (
            "progressive_ring_status",
            ProgressiveRingOrchestrator::new()
                .evaluate_without_rollout_state()
                .status,
        ),
    ]
}

#[test]
fn test_no_owned_gate_reports_passed_without_a_data_source() {
    // P1, stated once over all five so a partial fix cannot look complete.
    let tmp = tempfile::tempdir().expect("tempdir");
    let offenders: Vec<&str> = all_owned_statuses(tmp.path())
        .into_iter()
        .filter(|(_, s)| matches!(s, GateStatus::Passed | GateStatus::AutoUpdated))
        .map(|(n, _)| n)
        .collect();
    assert!(
        offenders.is_empty(),
        "I1: absent evidence is never a pass. Still reporting a pass: {offenders:?}"
    );
}

#[test]
fn test_no_owned_gate_fabricates_an_accusation_without_a_data_source() {
    // P3, the symmetric half. Over-correcting to Failed would block the fleet
    // on a defect nobody can reproduce, and the gate would get bypassed.
    let tmp = tempfile::tempdir().expect("tempdir");
    let offenders: Vec<&str> = all_owned_statuses(tmp.path())
        .into_iter()
        .filter(|(_, s)| matches!(s, GateStatus::Failed(_)))
        .map(|(n, _)| n)
        .collect();
    assert!(
        offenders.is_empty(),
        "I1 cuts both ways: with no measurement there is no defect to report. \
         Fabricating an accusation: {offenders:?}"
    );
}

#[test]
fn test_every_owned_gate_is_tracked_as_unmeasured() {
    // P5: the whole point of NotMeasured is that `unmeasured_gates` picks it up
    // and blocks merge admission. A gate that is honest but untracked is inert.
    let tmp = tempfile::tempdir().expect("tempdir");
    for (name, status) in all_owned_statuses(tmp.path()) {
        assert!(
            !status.is_measured(),
            "{name} claims to have produced a measurement it cannot have"
        );
        assert_eq!(
            status.unmeasured_gate_id(),
            Some(name),
            "{name} must be recoverable from its own status"
        );
    }
}

#[test]
fn test_unmeasured_gate_ids_match_the_fidelity_registry() {
    // P5: the gate_id is the join key between the status, the fidelity registry
    // and the PreMergeCertificationReport field. Drift here silently unblocks
    // merge admission, so it is checked by mechanism rather than by convention.
    let tmp = tempfile::tempdir().expect("tempdir");
    for (name, status) in all_owned_statuses(tmp.path()) {
        let id = status
            .unmeasured_gate_id()
            .unwrap_or_else(|| panic!("{name} is not NotMeasured: {status:?}"));
        let entry = anvil::fidelity::registry::AUDITED_GATES
            .iter()
            .find(|e| e.gate_id == id)
            .unwrap_or_else(|| {
                panic!(
                    "gate_id `{id}` has no entry in the fidelity registry; \
                     unmeasured_gates would name a gate nobody can look up"
                )
            });
        // `fidelity/mod.rs` states the rule -- "a gate at Aspirational fidelity
        // must report GateStatus::NotMeasured" -- and nothing enforced it. Both
        // directions are checked here so the registry and the status cannot
        // drift apart: an entry silently upgraded to Partial while still
        // measuring nothing would make `may_report_pass()` true again (I22).
        assert!(
            !entry.fidelity.may_report_pass(),
            "{id} reports NotMeasured but the registry declares it {}; a gate that \
             may report a pass must produce a measurement",
            entry.fidelity.label()
        );
        assert!(
            entry.blocked_on.is_some(),
            "{id} has no data source and must name what it is blocked on, so the \
             gap is closable rather than merely admitted"
        );
    }
}

#[test]
fn test_owned_gate_sources_declare_no_fabricated_measurement_constants() {
    // I22: one mechanism covering all five modules, so a sixth fabricated
    // constant added to any of them is caught without anyone remembering to
    // extend a per-gate test.
    let banned: &[(&str, &[&str])] = &[
        (
            "src/slo_canary_guard",
            &["simulated_", "burn_rate_1h", "burn_rate_6h"],
        ),
        (
            "src/remote_cache_optimizer",
            &["sample_metrics", "95.0", ".mock"],
        ),
        ("src/ci_wallclock_ratchet", &["142", "0.045", "ceiling!"]),
        // `baseline` is deliberately NOT banned in the shadow module: a real
        // mirror compares a candidate against a baseline response, and banning
        // the domain noun would block the implementation rather than the
        // fabrication (P3). The invented sample size and parity are banned by
        // value, and any *replacement* value is caught by the numeric-literal
        // scan below.
        (
            "src/cluster_state_auditor",
            &["replicas:", "_manifest = \""],
        ),
        ("src/shadow_traffic_harness", &["5000", "99.98"]),
        // The three gates the first pass of this lane did not reach. Their
        // constants sat in modules this table never named, which is how a
        // check written for exactly this defect class walked past three
        // instances of it.
        (
            "src/canary_rollout",
            &["= CanaryMetricsSnapshot {", "28.5", "150.0"],
        ),
        (
            "src/shuffle_shard_simulator",
            &[
                "tenant-primary",
                "tenant-secondary",
                "assigned_cells: vec!",
                "single_cell_outage_impact_ratio",
            ],
        ),
        (
            "src/progressive_rollout",
            &[
                "is_healthy",
                "impl Default for RolloutManifest",
                "us-east-1",
            ],
        ),
    ];
    for (dir, needles) in banned {
        assert_absent_from_module(dir, needles);
    }
}

#[test]
fn test_owned_gate_callers_assign_no_numeric_literal_measurements() {
    // The needle lists above catch the six constants named in the brief. They
    // do not catch the seventh: swapping `142` for `137`, or `5000` for `4711`,
    // satisfies every one of them and leaves the gate exactly as unfailable.
    //
    // This is the mechanism that does not need updating (I22): in the owned
    // modules -- the files that are supposed to OBTAIN a measurement, not state
    // one -- no numeric literal may be assigned into a variable or a struct
    // field at all. A measurement arrives from a data source; a literal is the
    // absence of one.
    //
    // Scoped to `mod.rs` when first written, which is half of why three further
    // instances of this defect survived it: `burn_rate_5m` was in a caller this
    // list did not name, and a sibling file was never scanned for numbers at
    // all. It now walks every `.rs` file under each owned module directory, the
    // same breadth `assert_absent_from_module` already had.
    // Reported over all five at once, so a partial fix cannot look complete.
    let mut offenders: Vec<String> = Vec::new();
    for dir in [
        "src/slo_canary_guard",
        "src/remote_cache_optimizer",
        "src/ci_wallclock_ratchet",
        "src/cluster_state_auditor",
        "src/shadow_traffic_harness",
        "src/canary_rollout",
        "src/shuffle_shard_simulator",
        "src/progressive_rollout",
    ] {
        for (rel, _) in module_production_sources(dir) {
            for hit in assigned_numeric_literals(&rel) {
                offenders.push(format!("{rel}: {hit}"));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "False Green prevention: {} numeric literal(s) assigned in production code, each a \
         measurement nobody took (I2): {offenders:#?}",
        offenders.len()
    );
}

// =========================================================================
// 6. CanaryRolloutGuard -- gate_id `canary_status`
//    Fabricated: `burn_rate_5m: 0.2` written four lines above its comparison
//    against `3.0`, and published as "5m burn rate 0.20x < 3.0x threshold".
//    Missing data source: a reachable Prometheus / OpenTelemetry endpoint and
//    a canary deployment to point it at.
//
//    Oracle: Argo Rollouts marks an unreachable provider `Error` and aborts the
//    rollout; Flagger refuses to start a canary when `IsOnline()` is false and
//    counts a no-data query as a failed check; Kayenta fails the canary at score
//    0 once half the metrics classify `Nodata`. Not one treats "could not
//    measure" as "measured healthy".
// =========================================================================

#[test]
fn test_canary_rollout_reports_not_measured_without_a_metrics_endpoint() {
    let status = CanaryRolloutGuard::new()
        .evaluate_without_metrics_source()
        .status;
    assert_not_measured(
        &status,
        "canary_status",
        &[
            "prometheus",
            "opentelemetry",
            "metrics",
            "canary deployment",
        ],
    );
    assert_no_accusation(&status);
}

/// P2/P6: the number and the sentence that published it must both be gone.
/// `0.2` re-valued to `0.15` defeats a needle list, so the numeric-literal scan
/// at the bottom of this file is the durable half; these needles pin the exact
/// strings the defect was written with, so a revert is loud.
#[test]
fn test_canary_rollout_false_green_prevention_burn_rate_constants_are_absent_from_source() {
    assert_absent_from_module(
        "src/canary_rollout",
        &[
            "= CanaryMetricsSnapshot {",
            "burn rate {:.2}x < 3.0x threshold",
            "28.5",
            "150.0",
        ],
    );
}

/// P4: the circuit breaker is the honest half and must survive. It is the seam
/// a real Prometheus query plugs into, and it has to be demonstrably able to
/// fail -- otherwise the gate satisfies its absence test by measuring nothing
/// and has no measuring path at all.
///
/// Pinned at, one below and one above the burn-rate ceiling, and again on p99,
/// so a mutant that flips `>` to `>=` or drops a branch is red.
#[test]
fn test_canary_rollout_measuring_path_still_trips_the_breaker_at_the_boundary() {
    let guard = CanaryRolloutGuard::new();
    let at = |burn: f64, p99: f64| {
        guard
            .evaluate_metrics_snapshot(
                &CanaryMetricsSnapshot {
                    step_traffic_percent: 5,
                    p99_latency_ms: p99,
                    error_rate_percent: 0.5,
                    burn_rate_5m: burn,
                },
                3.0,
                150.0,
            )
            .status
    };

    assert!(
        matches!(at(3.0, 100.0), GateStatus::Passed),
        "exactly at the burn-rate ceiling must pass -- the breaker trips above it"
    );
    assert!(
        matches!(at(3.01, 100.0), GateStatus::Failed(_)),
        "one above the burn-rate ceiling must FAIL"
    );
    assert!(
        matches!(at(2.99, 100.0), GateStatus::Passed),
        "one below the burn-rate ceiling must pass"
    );
    assert!(
        matches!(at(1.0, 150.01), GateStatus::Failed(_)),
        "one above the p99 ceiling must FAIL"
    );
    assert!(
        matches!(at(1.0, 150.0), GateStatus::Passed),
        "exactly at the p99 ceiling must pass"
    );
}

/// P1 restated for this gate: a measuring path that cannot tell a supplied
/// reading apart from an absent one is the defect wearing a new signature. The
/// failure reason must carry the reading it judged, and the report must carry
/// the snapshot, so a number on the scorecard traces to where it came from.
#[test]
fn test_canary_rollout_failure_names_the_reading_it_judged() {
    let report = CanaryRolloutGuard::new().evaluate_metrics_snapshot(
        &CanaryMetricsSnapshot {
            step_traffic_percent: 25,
            p99_latency_ms: 40.0,
            error_rate_percent: 4.0,
            burn_rate_5m: 14.4,
        },
        3.0,
        150.0,
    );
    let GateStatus::Failed(reason) = &report.status else {
        panic!("a burn rate of 14.4x against a 3.0x ceiling must FAIL: {report:?}");
    };
    assert!(
        reason.contains("14.4"),
        "the accusation must quote the reading it was made from; got: {reason}"
    );
    assert!(
        report.observed.is_some(),
        "a judged snapshot must be carried on the report"
    );
    assert!(
        CanaryRolloutGuard::new()
            .evaluate_without_metrics_source()
            .observed
            .is_none(),
        "nothing was read, so there is no snapshot to carry"
    );
}

// =========================================================================
// 7. ShuffleShardSimulator -- gate_id `shuffle_status`
//    Fabricated: a two-tenant topology written in the gate, eight cells, four
//    cells per tenant, tenants on [1,2,3,4] and [3,4,5,6]. Overlap is 2 on
//    every pull request against a `> 2` threshold, and "blast radius limited to
//    50.0%" is one literal divided by another.
//    Missing data source: a tenant-to-cell mapping table.
//
//    Oracle: the AWS Builders' Library gives blast radius as 1/C(n,k) -- the
//    fraction of the customer population sharing a tenant's whole shard --
//    which for n=8, k=4 is 1/70. Cells-per-tenant over total-cells is the
//    infrastructure footprint of one tenant, and it RISES as isolation improves
//    (Route 53 gives every domain four of 2048 name servers), so publishing it
//    as blast radius inverts the sign of the claim.
// =========================================================================

#[test]
fn test_shuffle_shard_reports_not_measured_without_a_topology_source() {
    let status = ShuffleShardSimulator::new()
        .evaluate_without_topology_source()
        .status;
    assert_not_measured(
        &status,
        "shuffle_status",
        &["topology", "tenant", "mapping", "control plane"],
    );
    assert_no_accusation(&status);
}

#[test]
fn test_shuffle_shard_false_green_prevention_topology_constants_are_absent_from_source() {
    assert_absent_from_module(
        "src/shuffle_shard_simulator",
        &[
            "tenant-primary",
            "tenant-secondary",
            "assigned_cells: vec!",
            // The sign-inverted metric. Cells-per-tenant over total-cells is not
            // blast radius under any AWS source; banning the identifier stops it
            // coming back under the name it was published as.
            "single_cell_outage_impact_ratio",
        ],
    );
}

/// P4: `ShuffleShardMath` is honest combinatorics and is the seam a real
/// mapping table plugs into. Pinned at, one below and one above the overlap
/// bound, over a topology the TEST supplies -- which is the whole point: the
/// same gate must pass and fail depending on its input, not on its source.
#[test]
fn test_shuffle_shard_measuring_path_still_fails_an_overlapping_topology() {
    let sim = ShuffleShardSimulator::new();
    let judge = |b: Vec<usize>| {
        sim.evaluate_topology(
            8,
            4,
            &[
                ShuffleShardAllocation {
                    tenant_id: "tenant-a".to_string(),
                    assigned_cells: vec![1, 2, 3, 4],
                },
                ShuffleShardAllocation {
                    tenant_id: "tenant-b".to_string(),
                    assigned_cells: b,
                },
            ],
            2,
        )
        .status
    };

    assert!(
        matches!(judge(vec![5, 6, 7, 8]), GateStatus::Passed),
        "disjoint shards overlap in 0 cells and must pass"
    );
    assert!(
        matches!(judge(vec![3, 4, 5, 6]), GateStatus::Passed),
        "exactly at the overlap bound must pass -- Route 53's guarantee is \
         'never MORE than two'"
    );
    assert!(
        matches!(judge(vec![2, 3, 4, 5]), GateStatus::Failed(_)),
        "one above the overlap bound -- three shared cells -- must FAIL"
    );
    assert!(
        matches!(judge(vec![1, 2, 3, 4]), GateStatus::Failed(_)),
        "two tenants on an identical shard is total overlap and must FAIL"
    );
}

/// P6 for this gate: the published blast radius must be the oracle's quantity,
/// not the one that reads well. C(8,4) is 70 shuffle shards, so the fraction of
/// the tenant population sharing any one tenant's whole shard is 1/70.
#[test]
fn test_shuffle_shard_publishes_the_blast_radius_the_oracle_defines() {
    let report = ShuffleShardSimulator::new().evaluate_topology(
        8,
        4,
        &[
            ShuffleShardAllocation {
                tenant_id: "tenant-a".to_string(),
                assigned_cells: vec![1, 2, 3, 4],
            },
            ShuffleShardAllocation {
                tenant_id: "tenant-b".to_string(),
                assigned_cells: vec![5, 6, 7, 8],
            },
        ],
        2,
    );
    let metrics = report.metrics.expect("a supplied topology yields metrics");
    assert_eq!(
        metrics.total_combinations, 70,
        "C(8,4) is 70 shuffle shards"
    );
    let expected = 1.0_f64 / 70.0;
    assert!(
        (metrics.uniform_random_shard_collision_ratio - expected).abs() < 1e-12,
        "blast radius is 1/C(n,k) = {expected}, not cells-per-tenant over total-cells \
         (which would be 0.5 here); got {}",
        metrics.uniform_random_shard_collision_ratio
    );
}

/// A topology that cannot exist is not a clean one, and an empty one is not a
/// measured one. Four cells per tenant out of two cells gives C(n,k) = 0, and
/// 1/0 is the `NaN` this repository already paid for once on the coverage gate.
#[test]
fn test_shuffle_shard_absent_evidence_an_impossible_or_empty_topology_is_not_a_pass() {
    let sim = ShuffleShardSimulator::new();

    let impossible = sim
        .evaluate_topology(
            2,
            4,
            &[ShuffleShardAllocation {
                tenant_id: "tenant-a".to_string(),
                assigned_cells: vec![1, 2],
            }],
            2,
        )
        .status;
    assert!(
        matches!(impossible, GateStatus::Errored(_)),
        "four cells per tenant out of two cells is not a topology; got {impossible:?}"
    );

    for (label, allocations) in [
        ("an empty topology", Vec::new()),
        (
            "a single tenant",
            vec![ShuffleShardAllocation {
                tenant_id: "tenant-a".to_string(),
                assigned_cells: vec![1, 2, 3, 4],
            }],
        ),
    ] {
        let status = sim.evaluate_topology(8, 4, &allocations, 2).status;
        assert!(
            !status.is_measured(),
            "{label} yields no pair of shards to compare, so the overlap bound was \
             never tested against anything -- an empty scope is not a pass (I1); \
             got {status:?}"
        );
        assert_no_accusation(&status);
    }
}

// =========================================================================
// 8. ProgressiveRingOrchestrator -- gate_id `progressive_ring_status`
//    Fabricated: the ring's health was a struct literal `true` in all four
//    match arms, driven by a constant that `NotMeasured.is_acceptable()` made
//    true. The two validators that check something real --
//    `validate_bake_window` and `validate_geo_paired_exclusion` -- had zero
//    production call sites.
//    Missing data source: rollout state -- an elapsed bake clock and the set of
//    regions currently receiving the rollout.
//
//    Oracle: Azure Safe Deployment Practices -- "Deployments must pass health
//    checks before each phase of progressive exposure can begin", bake times
//    "measured in hours and days rather than minutes", and region pairs updated
//    sequentially. An affirmative health check is required to advance; the
//    absence of a negative one is not one.
// =========================================================================

#[test]
fn test_progressive_ring_reports_not_measured_without_rollout_state() {
    let status = ProgressiveRingOrchestrator::new()
        .evaluate_without_rollout_state()
        .status;
    assert_not_measured(
        &status,
        "progressive_ring_status",
        &["rollout", "bake", "region", "deploy"],
    );
    assert_no_accusation(&status);
}

#[test]
fn test_progressive_ring_false_green_prevention_health_literal_is_absent_from_source() {
    assert_absent_from_module(
        "src/progressive_rollout",
        &[
            // The fabricated field itself, in all four arms.
            "is_healthy",
            // "Moved, not removed" into the manifest's own Default impl, which
            // is where the bake minutes and the region strings lived.
            "impl Default for RolloutManifest",
            // AWS region codes in a gate named for an Azure practice: Azure
            // pairs East US with West US, and AWS publishes no pair map at all.
            "us-east-1",
            "ap-northeast-1",
        ],
    );
}

/// P4, and the reason this gate is the interesting one: both validators were
/// written, unit-tested and never called. Wiring them is what gives the gate a
/// measuring path at all, so each is pinned here through the production entry
/// point rather than through the validator in isolation -- a helper with a unit
/// test and no caller is exactly what was already there.
#[test]
fn test_progressive_ring_measuring_path_still_holds_the_ring_on_a_short_bake() {
    let orch = ProgressiveRingOrchestrator::new();
    let manifest = bake_manifest();
    let regions = vec!["eastus".to_string()];
    let at = |elapsed: u64| {
        orch.evaluate_ring_advance(&DeploymentRing::Ring0Canary, elapsed, &regions, &manifest)
            .status
    };

    assert!(
        matches!(at(59), GateStatus::Failed(_)),
        "one minute short of the declared bake window must FAIL"
    );
    assert!(
        matches!(at(60), GateStatus::Passed),
        "exactly at the declared bake window must pass"
    );
    assert!(
        matches!(at(61), GateStatus::Passed),
        "one minute past the declared bake window must pass"
    );
}

#[test]
fn test_progressive_ring_measuring_path_still_refuses_a_paired_region_rollout() {
    let orch = ProgressiveRingOrchestrator::new();
    let manifest = bake_manifest();
    let judge = |regions: Vec<String>| {
        orch.evaluate_ring_advance(&DeploymentRing::Ring0Canary, 600, &regions, &manifest)
            .status
    };

    assert!(
        matches!(
            judge(vec!["eastus".to_string(), "northeurope".to_string()]),
            GateStatus::Passed
        ),
        "two regions that are not a pair may take the rollout together"
    );
    assert!(
        matches!(
            judge(vec!["eastus".to_string(), "westus".to_string()]),
            GateStatus::Failed(_)
        ),
        "East US and West US are an Azure region pair and are updated \
         sequentially, never together"
    );
    assert!(
        matches!(
            judge(vec!["northeurope".to_string(), "westeurope".to_string()]),
            GateStatus::Failed(_)
        ),
        "North Europe and West Europe are an Azure region pair"
    );
}

/// The advance itself must come from the manifest, not from a literal written
/// beside the match arm. The scheduler published a traffic percentage of 20 for
/// a ring the manifest declared at 25: two schedules, disagreeing, one of them
/// hardcoded.
#[test]
fn test_progressive_ring_traffic_percentage_comes_from_the_manifest() {
    let mut manifest = bake_manifest();
    manifest.rings[1].traffic_percentage = 7;

    let state = ProgressiveRingOrchestrator::new()
        .evaluate_ring_advance(
            &DeploymentRing::Ring0Canary,
            600,
            &["eastus".to_string()],
            &manifest,
        )
        .state
        .expect("an advancing ring carries the state it advances to");

    assert_eq!(state.target_ring, DeploymentRing::Ring1Dogfood);
    assert_eq!(
        state.traffic_pct, 7,
        "the traffic percentage must be read from the manifest's own RingConfig"
    );
}

/// A manifest that declares no config for the ring under evaluation is not a
/// satisfied bake window. The absence of a rule is not compliance with it --
/// the same inversion, one level down.
#[test]
fn test_progressive_ring_absent_evidence_an_undeclared_ring_is_not_baked() {
    let manifest = RolloutManifest {
        service_name: "svc".to_string(),
        geo_paired_exclusion_enabled: true,
        rings: Vec::new(),
    };
    let status = ProgressiveRingOrchestrator::new()
        .evaluate_ring_advance(
            &DeploymentRing::Ring0Canary,
            600,
            &["eastus".to_string()],
            &manifest,
        )
        .status;
    assert!(
        !matches!(status, GateStatus::Passed),
        "a ring the manifest never declares has no bake window to have satisfied; \
         got {status:?}"
    );
}

/// A rollout manifest is a deployment artefact a caller supplies. A `Default`
/// impl is a manifest the gate wrote for itself -- the fabrication in its most
/// respectable disguise, and where the bake minutes and the region strings
/// lived.
fn bake_manifest() -> RolloutManifest {
    RolloutManifest {
        service_name: "anvil".to_string(),
        geo_paired_exclusion_enabled: true,
        rings: vec![
            RingConfig {
                ring: DeploymentRing::Ring0Canary,
                traffic_percentage: 1,
                min_bake_minutes: 60,
                regions: vec!["eastus".to_string()],
            },
            RingConfig {
                ring: DeploymentRing::Ring1Dogfood,
                traffic_percentage: 5,
                min_bake_minutes: 360,
                regions: vec!["northeurope".to_string()],
            },
        ],
    }
}

// =========================================================================
// 9. The wiring. Three honest guards whose abstaining constructor nobody calls
//    are three gates that still publish a fabricated pass.
// =========================================================================

/// Catches the failure mode that ships this whole change green and changes
/// nothing: the guards learn to say `NotMeasured` and the certification
/// pipeline keeps calling the measuring entry point with values it wrote.
///
/// This is not hypothetical for these three. `progressive_ring_status` received
/// its constant as a *call argument* -- `aca_report.status.is_acceptable()`,
/// which is `true` for `NotMeasured` -- so it evaded
/// `no_gate_in_the_pipeline_is_invoked_with_a_fabricated_argument`, which looks
/// for literals. A gate with no data source must be called through a door that
/// takes no measurement at all.
#[test]
fn test_the_certification_pipeline_supplies_no_topology_metrics_or_rollout_state() {
    // Comments are stripped: the comment beside each of these calls explains why
    // the gate abstains and names the entry point it is NOT allowed to use, so a
    // scan over raw text would be satisfied by the prose and tripped by it in
    // turn.
    let src: String = production_source("src/webhook/pipelines/certify.rs")
        .lines()
        .map(code_only)
        .collect::<Vec<_>>()
        .join("\n");

    for (call, gate) in [
        ("evaluate_without_metrics_source()", "canary_status"),
        ("evaluate_without_topology_source()", "shuffle_status"),
        (
            "evaluate_without_rollout_state()",
            "progressive_ring_status",
        ),
    ] {
        assert!(
            src.contains(call),
            "{gate} has no data source, so the pipeline must reach it through \
             `{call}` rather than through a measuring entry point handed values \
             nobody read"
        );
    }

    for (call, why) in [
        (
            "evaluate_metrics_snapshot(",
            "a canary snapshot the pipeline can only have written itself",
        ),
        (
            "evaluate_topology(",
            "a tenant-to-cell topology the pipeline can only have written itself",
        ),
        (
            "evaluate_ring_advance(",
            "a bake clock and a live region set the pipeline cannot read",
        ),
    ] {
        assert!(
            !src.contains(call),
            "the certification pipeline calls `{call}`, which takes {why}"
        );
    }
}
