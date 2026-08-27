//! Ordering the queue from facts the items already carry.
//!
//! Nothing here may weigh or estimate. A score invented by the triage would be
//! a judgement wearing the costume of a measurement, and the queue's whole
//! value is that what it says can be checked against the items.

use anvil::intake::triage::{Urgency, triage};
use anvil::intake::{Queue, Remedy, Source, Subject, WorkItem};

fn item(source: Source, locus: &str, class: Option<&str>, remedy: Remedy) -> WorkItem {
    WorkItem {
        source,
        subject: Subject {
            repo: "oyatie/anvil".into(),
            locus: Some(locus.into()),
        },
        what: format!("finding at {locus}"),
        consequence: "something is lost".into(),
        class: class.map(str::to_string),
        remedy,
    }
}

fn mech() -> Remedy {
    Remedy::Mechanical {
        how: "run it".into(),
    }
}
fn judged() -> Remedy {
    Remedy::NeedsJudgement {
        why: "a person decides".into(),
    }
}

#[test]
fn urgency_is_read_off_the_source_not_assigned() {
    assert_eq!(
        Urgency::of(&item(Source::Incident, "a", None, mech())),
        Urgency::Live
    );
    assert_eq!(
        Urgency::of(&item(Source::Advisory, "a", None, mech())),
        Urgency::Security
    );
    assert_eq!(
        Urgency::of(&item(Source::Drift, "a", None, mech())),
        Urgency::Drifted
    );
    assert_eq!(
        Urgency::of(&item(Source::Audit, "a", None, mech())),
        Urgency::Standing
    );
}

#[test]
fn a_live_incident_outranks_everything_standing() {
    let mut q = Queue::new();
    q.raise(item(Source::Audit, "a", None, mech()));
    q.raise(item(Source::PostmortemRemedy, "b", None, mech()));
    q.raise(item(Source::Incident, "c", None, judged()));
    let t = triage(&q);
    assert_eq!(t.ordered[0].urgency, Urgency::Live);
    assert_eq!(t.ordered[0].item.subject.locus.as_deref(), Some("c"));
}

/// A class seen three times is not three problems. It is one rule that should
/// have been written after the first.
#[test]
fn recurrence_outranks_novelty_at_the_same_urgency() {
    let mut q = Queue::new();
    q.raise(item(Source::Audit, "solo", Some("rare-thing"), mech()));
    for n in 0..3 {
        q.raise(item(
            Source::Audit,
            &format!("f{n}"),
            Some("prose-read-as-code"),
            mech(),
        ));
    }
    let t = triage(&q);
    assert_eq!(
        t.ordered[0].recurrence, 3,
        "the repeated class did not lead"
    );
    assert_eq!(
        t.ordered[0].item.class.as_deref(),
        Some("prose-read-as-code")
    );
    assert_eq!(t.recurring_classes(), vec![("prose-read-as-code", 3)]);
}

/// An item naming no class must not be scored as if it were rare. Absence of a
/// class is an unknown count, not a count of one thing nobody has seen before.
#[test]
fn an_item_with_no_class_is_not_treated_as_a_rare_one() {
    let mut q = Queue::new();
    q.raise(item(Source::Audit, "unnamed", None, mech()));
    q.raise(item(Source::Audit, "a", Some("known"), mech()));
    q.raise(item(Source::Audit, "b", Some("known"), mech()));
    let t = triage(&q);
    assert_eq!(
        t.ordered[0].item.class.as_deref(),
        Some("known"),
        "an unclassed item outranked a class seen twice"
    );
    let unnamed = t.ordered.iter().find(|x| x.item.class.is_none()).unwrap();
    assert_eq!(unnamed.recurrence, 1);
}

#[test]
fn mechanical_work_precedes_judgement_at_the_same_rank() {
    let mut q = Queue::new();
    q.raise(item(Source::Audit, "needs-a-person", None, judged()));
    q.raise(item(Source::Audit, "just-run-it", None, mech()));
    let t = triage(&q);
    assert!(t.ordered[0].mechanical, "judgement was scheduled first");
}

/// The central refusal. An item nobody has classified cannot be placed: the
/// middle asserts a position never determined, and the bottom makes "nobody
/// looked" behave exactly like "this does not matter".
#[test]
fn unclassified_items_are_set_aside_rather_than_ranked() {
    let mut q = Queue::new();
    q.raise(item(Source::Incident, "urgent", None, mech()));
    q.raise(item(
        Source::Audit,
        "nobody-looked",
        None,
        Remedy::Unclassified,
    ));
    let t = triage(&q);
    assert_eq!(t.ordered.len(), 1, "an unclassified item was ranked");
    assert_eq!(t.unclassified.len(), 1);
    assert_eq!(
        t.unclassified[0].subject.locus.as_deref(),
        Some("nobody-looked")
    );
    assert!((t.unclassified_share() - 0.5).abs() < f64::EPSILON);
}

/// An order that shuffles between runs is one nobody can work through.
#[test]
fn the_order_is_stable_across_runs() {
    let mut q = Queue::new();
    for n in 0..8 {
        q.raise(item(Source::Audit, &format!("f{n}"), Some("same"), mech()));
    }
    let a: Vec<String> = triage(&q).ordered.iter().map(|t| t.item.id()).collect();
    let b: Vec<String> = triage(&q).ordered.iter().map(|t| t.item.id()).collect();
    assert_eq!(a, b, "two triages of one queue disagreed on the order");
}

#[test]
fn an_empty_queue_reports_no_unclassified_share() {
    let empty = Queue::new();
    let t = triage(&empty);
    assert!(t.ordered.is_empty());
    assert_eq!(
        t.unclassified_share(),
        0.0,
        "an empty queue divided by zero"
    );
}
