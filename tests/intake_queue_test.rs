//! What the queue guarantees, whatever raises the work.
//!
//! Seven producers raise work in this codebase and each raised its own shape,
//! so nothing could compare across them or say what was outstanding. These are
//! the properties that hold once they share one: an identity that converges, a
//! classification that replaces rather than forks, and a distinction between
//! "nobody looked" and "a machine cannot".
//!
//! The producers themselves are in `intake_producers_test.rs`. The seam is
//! real rather than arithmetic: nothing here imports a producing module, and
//! everything there does.

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
    let module = "src/intake";
    let code = anvil::source_scan::code_only(&anvil::source_scan::paths::module_source(
        module,
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
    ));
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
            "{module} imports `{producer}`. `intake` is shared vocabulary and \
             must stay a leaf: a module every producer imports, importing \
             every producer back, is the hub-and-spoke shape that put \
             seventy modules into one dependency cycle."
        );
    }
}
