//! What the audits find is raised, so it re-enters the system.
//!
//! `intake::Queue` and all six `work_items()` producers had zero production
//! callers. Every audit pass printed its findings and queued none of them —
//! the arc `intake`'s own module doc names as its reason to exist: *a finding
//! printed and not queued is a finding that will be found again.*
//!
//! Two halves are needed and only one is the plumbing. The producers must be
//! raised, and the sweep must consult them each pass — a queue built once at
//! start-up is a snapshot, not a backlog.

use anvil::cli::intake_sweep::{AuditInputs, raise_for_repo};

/// A corpus report with one unauthorised SSOT claim, which is a real finding
/// the auditor raises.
fn a_corpus_finding() -> anvil::corpus_auditor::CorpusAuditReport {
    let mut r = an_empty_corpus_report();
    r.unauthorized_ssot_claims = vec!["docs/invented-ssot.md".to_string()];
    r
}

fn an_empty_corpus_report() -> anvil::corpus_auditor::CorpusAuditReport {
    anvil::corpus_auditor::CorpusAuditReport {
        total_files: 1,
        freshness_ratio: 1.0,
        dormant_files_count: 0,
        stale_adrs_count: 0,
        unauthorized_ssot_claims: Vec::new(),
        frontmatter_violations: Vec::new(),
        summary: String::new(),
    }
}

#[test]
fn the_postmortem_ledger_reaches_the_queue() {
    let raised = raise_for_repo("oyatie/anvil", &AuditInputs::default());
    assert!(
        raised.by_producer.contains_key("postmortem"),
        "the postmortem producer must run every sweep: {:?}",
        raised.by_producer
    );
    assert!(
        !raised.queue.is_empty(),
        "the postmortem ledger records remedies that do not exist yet, and \
         those are outstanding work. An empty queue here means the producer \
         ran and nothing reached the queue."
    );
}

/// The distinction the whole codebase turns on: "we did not look" is not
/// "we looked and found nothing".
#[test]
fn a_corpus_audit_that_could_not_run_is_absent_not_zero() {
    let could_not_look = raise_for_repo("oyatie/anvil", &AuditInputs::default());
    assert!(
        !could_not_look.by_producer.contains_key("corpus_auditor"),
        "an audit that did not run must be absent from the record, not a zero"
    );

    let looked_and_found_nothing = raise_for_repo(
        "oyatie/anvil",
        &AuditInputs {
            corpus: Some(&an_empty_corpus_report()),
            ..Default::default()
        },
    );
    assert_eq!(
        looked_and_found_nothing.by_producer.get("corpus_auditor"),
        Some(&0),
        "an audit that ran and found nothing is a zero, which is evidence"
    );
}

#[test]
fn a_corpus_finding_is_raised_as_work() {
    let empty = raise_for_repo(
        "oyatie/anvil",
        &AuditInputs {
            corpus: Some(&an_empty_corpus_report()),
            ..Default::default()
        },
    );
    let found = raise_for_repo(
        "oyatie/anvil",
        &AuditInputs {
            corpus: Some(&a_corpus_finding()),
            ..Default::default()
        },
    );
    assert_eq!(
        found.by_producer.get("corpus_auditor"),
        Some(&1),
        "one unauthorised SSOT claim is one item of work"
    );
    assert_eq!(
        found.queue.len(),
        empty.queue.len() + 1,
        "and it reaches the queue rather than being counted and dropped"
    );
}

/// Identity is derived, so a sweep that sees the same finding again does not
/// grow the backlog. Without this the queue grows linearly with the number of
/// sweeps and never converges — the exact defect that made the recovery sweep
/// re-certify every open pull request on every pass.
#[test]
fn raising_the_same_finding_twice_yields_one_item() {
    let once = raise_for_repo(
        "oyatie/anvil",
        &AuditInputs {
            corpus: Some(&a_corpus_finding()),
            ..Default::default()
        },
    );
    let twice = {
        let mut q = anvil::intake::Queue::new();
        let report = a_corpus_finding();
        for _ in 0..2 {
            for item in report.work_items("oyatie/anvil") {
                q.raise(item);
            }
        }
        q
    };
    assert_eq!(
        twice.len(),
        1,
        "the same finding raised twice is one item, by derived identity"
    );
    assert!(!once.queue.is_empty());
}

/// The queue is worth what the sweep does with it, so the wiring is asserted
/// too — keyed to the call, and loud if the call is no longer there to find.
#[test]
fn the_hourly_sweep_raises_every_pass() {
    let src = anvil::source_scan::paths::module_source(
        "src/cli/sweep_task",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );

    let raise = src
        .find("intake_sweep::raise_for_repo(")
        .unwrap_or_else(|| {
            panic!(
                "nothing in the hourly sweep raises into the queue. If that moved, \
             this test must follow it -- a scan that stops finding its subject \
             is not a fix."
            )
        });
    let loop_at = src
        .find("for repo in &repos")
        .expect("the sweep still iterates the watched repositories");
    assert!(
        loop_at < raise,
        "the raise happens outside the per-repository loop, so it runs once \
         rather than every pass"
    );
    assert!(
        src[raise..].contains("record_work_queue("),
        "the backlog is raised and never recorded, so nothing downstream can \
         see it move"
    );
}

/// Two producers are deliberately not wired, and the reason must stay visible:
/// each is a hardcoded constant today, and raising a constant would make the
/// queue's depth a statement about nothing.
#[test]
fn the_two_unwirable_producers_are_named_rather_than_silently_skipped() {
    let src = anvil::source_scan::paths::module_source(
        "src/cli/intake_sweep",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    for named in ["incident_sentry", "review_memory"] {
        assert!(
            src.contains(named),
            "{named} raises nothing today and the module must say why, or the \
             gap reads as an oversight and gets wired blind"
        );
    }

    // And the claim itself, measured rather than asserted in prose.
    let sentry = anvil::source_scan::paths::module_source(
        "src/incident_sentry",
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    );
    assert!(
        sentry.contains("fn live_golden_signals"),
        "if the sentry gained a real signal source, this exemption is stale \
         and the producer should be wired"
    );
}
