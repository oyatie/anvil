//! Issue #53: three daemon lookups that could resolve nothing and had no way
//! to say so, so each invented a value a reader takes for an observation.
//!
//! The fleet poller published the string `HEAD` in a commit field when neither
//! `dev` nor `main` was fetched. The dashboard dropped a failed open-pull-request
//! query with `if let Ok(..)`, so a rate-limited call rendered as an idle merge
//! queue. The flake rehabilitation subcommand handed the lifecycle
//! `tests::flaky_test`, a name that is not a test in this repository, and
//! printed the outcome as this repository's.
//!
//! The fix in all three is the type. `String`, `Vec` and a literal argument
//! cannot spell "not observed", so every path that failed to observe had to
//! invent something plausible.
//!
//! # Premortem
//!
//! Assume this change has already failed. The ways it can have, each a test:
//!
//! P1. The fallback moves rather than goes: `Option` on the field, and a
//!     `unwrap_or_else` at the call site or in a formatter puts `HEAD` back on
//!     the surface. -> `no_published_head_sha_field_falls_back_to_a_literal`
//!     scans both modules' production source, so a fallback anywhere in either
//!     is caught wherever it is written.
//! P2. The absence is representable but never rendered: `None` reaches the
//!     surface and serializes as something a reader still reads as a commit.
//!     -> `an_unresolved_head_sha_is_published_as_absent`.
//! P3. Over-correction (I1's other direction): a repo whose branches DID
//!     resolve stops publishing its head SHA, so a fleet that was observed
//!     reads as unobserved. -> the `Some` arms of
//!     `a_branch_lookup_that_resolved_nothing_yields_no_head_sha`, and the
//!     observed-repo arm of `a_failed_open_pull_request_query_is_not_an_idle_queue`.
//! P4. The failed PR fetch is recorded but the panel still says the queue is
//!     idle, so the operator reads the same sentence either way.
//!     -> `a_failed_open_pull_request_query_is_not_an_idle_queue`.
//! P5. The rehabilitation arm stops naming `tests::flaky_test` and names some
//!     other test instead, or reports counts for a set it never read.
//!     -> `the_rehabilitation_subcommand_names_no_test_of_its_own` bans any
//!     quarantine-member literal in the subcommand tree rather than that one
//!     string, and `rehabilitation_reports_an_absent_ledger` asserts the
//!     report carries no counts when there is nothing to count.
//! P6. A scan reads the wrong file, or nothing at all, and reports nothing
//!     wrong. -> every scan asserts an anchor it must be able to see, and
//!     `module_source` panics when its subject has no production source.

use std::collections::HashMap;
use std::path::Path;

use anvil::dashboard::panel_formatters::build_merge_train_rows;
use anvil::dashboard::{DashboardStateView, FleetRepoView};
use anvil::flake_quarantine::FlakeQuarantineLifecycle;
use anvil::fleet_observer::resolve_head_sha;

/// Production source for a module, keyed to the module rather than to a file.
/// Splitting an over-budget file into a directory is routine here, and a
/// path-keyed read goes blind -- not red -- the day it happens.
fn module_source(module: &str) -> String {
    anvil::source_scan::paths::module_source(module, Path::new(env!("CARGO_MANIFEST_DIR")))
}

fn repo_view(head_sha: Option<&str>) -> FleetRepoView {
    FleetRepoView {
        name: "oyatie/anvil".to_string(),
        head_sha: head_sha.map(str::to_string),
        open_prs: 0,
        pass_rate: 0.0,
        lead_time_hours: 0.0,
        deploy_frequency_per_day: 0.0,
        branch_shas: HashMap::new(),
        gate_failures: Vec::new(),
    }
}

/// `resolve_head_sha` is the whole of the poller's head-SHA decision, so the
/// unresolved case is reachable without a `GitHubClient`.
#[test]
fn a_branch_lookup_that_resolved_nothing_yields_no_head_sha() {
    assert_eq!(
        resolve_head_sha(&HashMap::new()),
        None,
        "a branch fetch that returned nothing resolved no commit to publish"
    );

    let mut shas = HashMap::new();
    shas.insert("staging".to_string(), "5555555".to_string());
    assert_eq!(
        resolve_head_sha(&shas),
        None,
        "neither `dev` nor `main` resolved, so there is no active-branch head"
    );

    shas.insert("main".to_string(), "2222222".to_string());
    assert_eq!(
        resolve_head_sha(&shas).as_deref(),
        Some("2222222"),
        "`main` resolved: a fetch that DID observe must still publish"
    );

    shas.insert("dev".to_string(), "1111111".to_string());
    assert_eq!(
        resolve_head_sha(&shas).as_deref(),
        Some("1111111"),
        "`dev` is preferred over `main` when both resolved"
    );
}

/// The view is published verbatim on `/api/state`, where `head_sha` sits in
/// the field a reader takes for a commit.
#[test]
fn an_unresolved_head_sha_is_published_as_absent() {
    let unobserved = DashboardStateView {
        fleet_repos: vec![repo_view(None)],
        ..Default::default()
    };
    let json = serde_json::to_string(&unobserved).expect("serialize");
    assert!(
        json.contains("\"head_sha\":null"),
        "an unresolved head SHA must publish as absent, not as a string: {json}"
    );
    assert!(
        !json.contains("HEAD"),
        "`HEAD` is not a commit, and this field is read as one: {json}"
    );

    let observed = DashboardStateView {
        fleet_repos: vec![repo_view(Some("abc1234"))],
        ..Default::default()
    };
    let json = serde_json::to_string(&observed).expect("serialize");
    assert!(
        json.contains("abc1234"),
        "a resolved head SHA must still reach the surface: {json}"
    );
}

/// P1: an `Option` on the field is undone by a fallback anywhere downstream.
#[test]
fn no_published_head_sha_field_falls_back_to_a_literal() {
    for module in ["src/fleet_observer", "src/dashboard"] {
        let src = module_source(module);
        assert!(
            src.contains("head_sha"),
            "{module}: this scan must be able to see the field it judges"
        );
        for needle in ["\"HEAD\"", "'HEAD'"] {
            assert!(
                !src.contains(needle),
                "{module}: `{needle}` published in a commit field is a lookup that \
                 resolved nothing wearing the shape of one that did"
            );
        }
    }
}

/// A rate-limited query and an idle queue both leave the merge train empty.
#[test]
fn a_failed_open_pull_request_query_is_not_an_idle_queue() {
    let unobserved = DashboardStateView {
        merge_train: Vec::new(),
        unobserved_merge_train_repos: vec!["oyatie/anvil".to_string()],
        ..Default::default()
    };
    let rendered = build_merge_train_rows(&unobserved);
    assert!(
        !rendered.contains("Queue idle"),
        "a query that never answered is not an observation of an idle queue: {rendered}"
    );
    assert!(
        rendered.contains("not observed") && rendered.contains("oyatie/anvil"),
        "the panel must name the repo it could not observe: {rendered}"
    );

    let observed = DashboardStateView::default();
    let rendered = build_merge_train_rows(&observed);
    assert!(
        rendered.contains("Queue idle"),
        "a queue that WAS observed and is empty must still read as idle: {rendered}"
    );
}

/// P1 again, for the fetch this time: `if let Ok(..)` drops the error arm, so
/// a failed query and an empty answer become the same empty vector.
#[test]
fn the_dashboard_does_not_swallow_a_failed_pull_request_fetch() {
    let src = module_source("src/dashboard");
    assert!(
        src.contains("list_open_prs"),
        "this scan must be able to see the fetch it judges"
    );
    assert!(
        !src.contains("if let Ok(open_prs"),
        "a dropped error arm renders a rate-limited call as an empty merge train"
    );
    assert!(
        src.contains("unobserved_merge_train_repos.push("),
        "the error arm must record the repo it failed on. Declaring the field \
         and never writing to it publishes the same empty merge train as the \
         `if let Ok(..)` it replaced"
    );
}

/// Nothing in this repository records quarantine membership, so the honest
/// answer to "what is quarantined" is that there is no ledger to read.
#[test]
fn rehabilitation_reports_an_absent_ledger() {
    let lifecycle = FlakeQuarantineLifecycle::new();
    assert_eq!(
        lifecycle.retained_quarantine_set(),
        None,
        "no run history and no quarantine lane means no ledger, not an empty one"
    );

    let report = lifecycle.rehabilitation_report();
    assert!(
        report.contains("nothing to rehabilitate"),
        "the operator must be told there is nothing to act on: {report}"
    );
    assert!(
        !report.contains("Quarantined:"),
        "counts over a set that was never read are counts over nothing: {report}"
    );
}

/// P5: the subcommand must take its input from the quarantine set, not supply
/// one. Any member literal here is a test this repository did not quarantine.
#[test]
fn the_rehabilitation_subcommand_names_no_test_of_its_own() {
    let src = module_source("src/cli");
    assert!(
        src.contains("FlakeRehab"),
        "this scan must be able to see the subcommand it judges, whichever way \
         its arm is written"
    );
    for needle in ["flaky", "non_deterministic"] {
        assert!(
            !src.contains(needle),
            "`{needle}` in the subcommand tree is a quarantine member the CLI \
             invented, and the printed outcome reads as this repository's"
        );
    }
}
