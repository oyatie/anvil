//! A circuit breaker with no telemetry has not found the system healthy.
//!
//! `evaluate_incident_sentry` is documented as a "100% deterministic evaluation
//! of live production incident health". It built its own `LiveGoldenSignals`
//! from four literals -- `p99_latency_ms: 64.0`, `error_rate_pct: 0.002`,
//! `panic_count_last_5m: 0` -- and fed them to a threshold function whose
//! limits are 500ms, 0.5% and 0 panics. Every literal sat comfortably inside
//! every budget, so the breaker reported healthy on every pull request and
//! could not trip on any of them.
//!
//! It was found by widening an existing guard. That guard already forbade a
//! literal latency, and had been scoped to `src/local_inner_loop` -- the
//! directory where the defect was first found rather than the tree where it can
//! occur.

use anvil::git_manager::PrDiffContext;
use anvil::incident_sentry::IncidentSentryCircuitBreaker;
use std::path::Path;

fn ctx() -> PrDiffContext {
    PrDiffContext {
        repo: "oyatie/anvil".to_string(),
        pr_number: 1,
        base_branch: "dev".to_string(),
        base_sha: "aaa".to_string(),
        head_sha: "deadbeef".to_string(),
        diff_content: String::new(),
        changed_files: vec![],
        repo_working_dir: std::path::PathBuf::from("."),
        is_incremental: false,
        previous_head_sha: None,
    }
}

#[test]
fn with_no_telemetry_the_sentry_does_not_report_health() {
    let r = IncidentSentryCircuitBreaker::new()
        .evaluate_incident_sentry(Path::new("."), &ctx())
        .expect("sentry runs");
    assert!(
        !r.measured,
        "no telemetry endpoint is configured, so nothing was read"
    );
    assert!(
        !r.is_healthy,
        "absence of data is not evidence of health. This assertion IS the \
         defect: the sentry returned is_healthy for every change ever put \
         through it."
    );
}

#[test]
fn an_unmeasured_sentry_never_triggers_an_emergency_revert() {
    let r = IncidentSentryCircuitBreaker::new()
        .evaluate_incident_sentry(Path::new("."), &ctx())
        .expect("sentry runs");
    assert!(
        !r.should_revert,
        "reverting on absent data is the opposite failure and a far more \
         destructive one: the remedy is an automatic `git revert`"
    );
}

#[test]
fn the_summary_names_what_is_missing_rather_than_claiming_a_pass() {
    let r = IncidentSentryCircuitBreaker::new()
        .evaluate_incident_sentry(Path::new("."), &ctx())
        .expect("sentry runs");
    assert!(r.summary.contains("NOT MEASURED"), "got: {}", r.summary);
    assert!(
        r.summary.contains("telemetry"),
        "an absence that does not name the missing capability cannot be acted \
         on. Got: {}",
        r.summary
    );
    assert!(
        !r.summary.contains("healthy"),
        "the previous summary said 'Production golden signals healthy'. Got: {}",
        r.summary
    );
}

#[test]
fn the_threshold_function_itself_still_discriminates() {
    // The decision function was never the defect -- only its fabricated input.
    // If this stops holding, the fix removed real capability.
    use anvil::incident_sentry::telemetry_sentry::{LiveGoldenSignals, TelemetrySentry};
    let breaching = LiveGoldenSignals {
        p99_latency_ms: 900.0,
        error_rate_pct: 4.0,
        panic_count_last_5m: 3,
        deployed_commit_sha: "abc".to_string(),
    };
    let d = TelemetrySentry::new().evaluate_production_health(&breaching);
    assert!(!d.is_healthy && d.should_emergency_revert);
}

#[test]
fn an_unmeasured_sentry_raises_no_work_item() {
    let r = IncidentSentryCircuitBreaker::new()
        .evaluate_incident_sentry(Path::new("."), &ctx())
        .expect("sentry runs");
    assert!(
        r.work_items("oyatie/anvil").is_empty(),
        "`work_items` keyed on `is_healthy`, which is now false for an \
         unmeasured run as well as a breaching one. Without the `measured` \
         guard every sweep would queue a standing 'the deployment is not \
         healthy' item describing a measurement that never happened."
    );
}
