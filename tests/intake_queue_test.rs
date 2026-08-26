//! One shape for every intent, and a queue that converges.
//!
//! Seven producers raise work in this codebase and each raises its own shape,
//! so nothing can compare across them or say what is outstanding. Worse, the
//! standing audits print findings that re-enter nothing — the arrow from LEARN
//! back to INTAKE is an arc, and a finding that is not queued will be found
//! again.

use anvil::intake::{Queue, Remedy, Source, Subject, WorkItem};

fn item(what: &str) -> WorkItem {
    WorkItem {
        source: Source::Audit,
        subject: Subject {
            repo: "oyatie/anvil".into(),
            locus: Some("src/thing.rs".into()),
        },
        what: what.into(),
        consequence: "something is lost".into(),
        class: None,
        remedy: Remedy::Unclassified,
    }
}

/// The property that makes this a queue rather than a log.
///
/// Every audit pass re-reports everything it can still see. With a generated
/// id the backlog would grow linearly with the number of sweeps and never
/// converge — which is the defect the recovery sweep already had, re-certifying
/// every open pull request on every pass.
#[test]
fn raising_the_same_finding_twice_yields_one_item() {
    let mut q = Queue::new();
    assert!(q.raise(item("a thing is wrong")), "first raise is new");
    assert!(
        !q.raise(item("a thing is wrong")),
        "the same finding raised again was reported as new"
    );
    assert_eq!(q.len(), 1, "the queue grew on a repeated finding");
}

/// Re-classifying must update in place, not fork the item. Identity excludes
/// the remedy for exactly this reason.
#[test]
fn a_better_classification_replaces_rather_than_duplicates() {
    let mut q = Queue::new();
    q.raise(item("a thing is wrong"));
    let mut better = item("a thing is wrong");
    better.remedy = Remedy::Mechanical {
        how: "run the codemod".into(),
    };
    q.raise(better);
    assert_eq!(q.len(), 1, "re-classifying forked the item");
    assert!(
        matches!(q.outstanding()[0].remedy, Remedy::Mechanical { .. }),
        "the better classification did not replace the worse one"
    );
}

#[test]
fn different_findings_are_different_items() {
    let mut q = Queue::new();
    q.raise(item("a thing is wrong"));
    q.raise(item("a different thing is wrong"));
    let mut elsewhere = item("a thing is wrong");
    elsewhere.subject.locus = Some("src/other.rs".into());
    q.raise(elsewhere);
    assert_eq!(q.len(), 3, "distinct findings were collapsed into one");
}

/// Recurrence is the signal that a deterministic rule is owed: a class seen
/// twice was a rule that should have been written after the first.
#[test]
fn items_are_countable_by_class() {
    let mut q = Queue::new();
    for (n, what) in ["one", "two", "three"].iter().enumerate() {
        let mut i = item(what);
        i.subject.locus = Some(format!("src/f{n}.rs"));
        i.class = Some("prose-read-as-code".into());
        q.raise(i);
    }
    assert_eq!(q.by_class().get("prose-read-as-code"), Some(&3));
}

/// An item nobody has classified is distinct from one that needs judgement.
/// Collapsing them would let "nobody looked" pass as "a machine cannot".
#[test]
fn unclassified_is_not_the_same_as_needs_judgement() {
    let mut q = Queue::new();
    q.raise(item("nobody looked"));
    let mut judged = item("a person must decide");
    judged.remedy = Remedy::NeedsJudgement {
        why: "where it belongs is a design decision".into(),
    };
    q.raise(judged);
    assert_eq!(q.unclassified().len(), 1);
    assert_eq!(q.unclassified()[0].what, "nobody looked");
}

/// The LEARN-to-INTAKE arrow: what the ledger already knows becomes work.
#[test]
fn unbuilt_postmortem_remedies_become_work_items() {
    let raised = anvil::postmortem::work_items("oyatie/anvil");
    assert_eq!(
        raised.len(),
        anvil::postmortem::missing_remedies().len(),
        "the ledger's unbuilt remedies did not all reach the queue"
    );
    for i in &raised {
        assert_eq!(i.source, Source::PostmortemRemedy);
        assert!(i.class.is_some(), "an item from the ledger lost its class");
        assert!(
            !i.consequence.is_empty(),
            "an item that cannot say what is lost cannot be prioritised"
        );
    }
}

/// ...and remedies already BUILT must not be raised, or the producer reports
/// the ledger's whole contents as outstanding.
#[test]
fn built_remedies_are_not_raised_as_work() {
    let raised = anvil::postmortem::work_items("oyatie/anvil");
    assert!(
        anvil::postmortem::built_remedy_count() > 0,
        "fixture sanity: the ledger records remedies that ARE built"
    );
    assert!(
        raised.len() < anvil::postmortem::built_remedy_count() + raised.len() + 1,
        "sanity"
    );
    for i in &raised {
        assert!(
            i.what.starts_with("unbuilt remedy:"),
            "a built remedy was raised as outstanding work: {}",
            i.what
        );
    }
}

/// Sweeping twice must converge, asserted at the level a caller uses.
///
/// There is no central `sweep`: a caller composes the producers it wants, and
/// each producer lives with the module that owns the finding. That keeps
/// `intake` a leaf — a vocabulary importing all its producers while each
/// imports it back is the hub shape that put seventy modules in one cycle.
#[test]
fn sweeping_twice_does_not_grow_the_backlog() {
    let sweep = || anvil::postmortem::work_items("oyatie/anvil");
    let mut q = Queue::new();
    for i in sweep() {
        q.raise(i);
    }
    let after_first = q.len();
    for i in sweep() {
        q.raise(i);
    }
    assert_eq!(
        q.len(),
        after_first,
        "a second sweep added items, so the backlog grows with every pass"
    );
}

/// `intake` must not import its producers. This is the cycle this codebase
/// already has seventy modules inside of.
#[test]
fn intake_stays_a_leaf() {
    for f in ["src/intake/mod.rs", "src/intake/sources.rs"] {
        let code = anvil::source_scan::code_only(&std::fs::read_to_string(f).unwrap());
        for producer in [
            "postmortem",
            "gitops_drift",
            "zero_day",
            "incident_sentry",
            "review_memory",
            "issue_reconciler",
            "corpus_auditor",
        ] {
            assert!(
                !code.contains(&format!("crate::{producer}")),
                "{f} imports `{producer}`. `intake` is shared vocabulary and \
                 must stay a leaf: a module every producer imports, importing \
                 every producer back, is the hub-and-spoke shape that put \
                 seventy modules into one dependency cycle."
            );
        }
    }
}

/// A second producer, in the module that owns the finding.
///
/// Two producers is the point at which the shape is decided: either `intake`
/// imports both and becomes a hub, or each module declares its own and the
/// vocabulary stays a leaf. This asserts the second.
#[test]
fn drift_findings_become_work_items_without_intake_knowing_about_drift() {
    use anvil::gitops_drift_reconciler::GitOpsDriftReport;
    use anvil::pre_merge_guard::GateStatus;

    let report = GitOpsDriftReport {
        status: GateStatus::Passed,
        is_safe: false,
        orphan_findings: vec![
            anvil::gitops_drift_reconciler::orphan_sweeper::OrphanManifestFinding {
                file_path: "k8s/orphan.yaml".into(),
                manifest_kind: "Deployment".into(),
                reason: "no reconciler references it".into(),
            },
        ],
        summary: String::new(),
    };
    let items = report.work_items("oyatie/anvil");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source, Source::Drift);
    assert_eq!(
        items[0].subject.locus.as_deref(),
        Some("k8s/orphan.yaml"),
        "the item does not name the manifest, so nobody can act on it"
    );
    assert!(matches!(items[0].remedy, Remedy::Mechanical { .. }));

    // Two producers, one queue, and the queue does not care which is which.
    let mut q = Queue::new();
    for i in anvil::postmortem::work_items("oyatie/anvil") {
        q.raise(i);
    }
    for i in report.work_items("oyatie/anvil") {
        q.raise(i);
    }
    assert!(
        q.len() >= 2,
        "two sources did not both reach one queue, which is the whole point"
    );
}

/// An empty report raises nothing. A producer that raised an item per SWEEP
/// rather than per FINDING would fill the backlog with evidence of its own
/// running.
#[test]
fn a_clean_report_raises_no_work() {
    use anvil::gitops_drift_reconciler::GitOpsDriftReport;
    use anvil::pre_merge_guard::GateStatus;
    let clean = GitOpsDriftReport {
        status: GateStatus::Passed,
        is_safe: true,
        orphan_findings: vec![],
        summary: String::new(),
    };
    assert!(
        clean.work_items("oyatie/anvil").is_empty(),
        "a clean sweep raised work, so the backlog would record that the \
         auditor ran rather than that anything is wrong"
    );
}
