//! A plan is worth having because it can be refused before anyone writes a
//! line. Every refusal here is free now and expensive later: overlapping
//! write-sets become quadratic consolidation, and a dependency loop becomes a
//! stack of pull requests that lands in no order at all.

use std::collections::BTreeSet;

use anvil::plan::{Plan, Refusal, conflicts, waves};

fn set(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

fn plan(id: &str, writes: &[&str], deps: &[&str]) -> Plan {
    Plan {
        item_id: id.into(),
        write_set: set(writes),
        adds_edges: BTreeSet::new(),
        depends_on: set(deps),
    }
}

#[test]
fn disjoint_independent_plans_share_one_wave() {
    let plans = vec![
        plan("a", &["src/a.rs"], &[]),
        plan("b", &["src/b.rs"], &[]),
        plan("c", &["src/c.rs"], &[]),
    ];
    let w = waves(&plans).expect("no reason to refuse");
    assert_eq!(w.len(), 1, "independent disjoint work was serialised");
    assert_eq!(w[0].len(), 3);
}

/// The refusal that pays for itself. Two lanes on one path is the conflict
/// that makes consolidating N worktrees quadratic.
#[test]
fn plans_that_write_the_same_path_are_put_in_different_waves() {
    let plans = vec![
        plan("a", &["src/shared.rs"], &[]),
        plan("b", &["src/shared.rs"], &[]),
    ];
    let w = waves(&plans).expect("sequencing is not refusal");
    assert_eq!(w.len(), 2, "two plans on one path were run together");
    assert_eq!(w[0].len(), 1);
    assert_eq!(w[1].len(), 1);
}

/// Holding a plan back is not refusing it. The distinction matters: a
/// sequenced plan still lands, a refused one needs a person.
#[test]
fn a_held_back_plan_still_lands_in_a_later_wave() {
    let plans = vec![
        plan("a", &["src/shared.rs", "src/a.rs"], &[]),
        plan("b", &["src/shared.rs"], &[]),
        plan("c", &["src/c.rs"], &[]),
    ];
    let w = waves(&plans).expect("sequencing");
    let landed: Vec<&str> = w.iter().flatten().map(|p| p.item_id.as_str()).collect();
    assert_eq!(landed.len(), 3, "a plan was dropped rather than sequenced");
    assert!(landed.contains(&"b"));
}

#[test]
fn dependencies_are_respected_across_waves() {
    let plans = vec![
        plan("adapter", &["src/adapters/x.rs"], &["port"]),
        plan("port", &["src/ports/x.rs"], &[]),
        plan("facade", &["src/facade/x.rs"], &["adapter"]),
    ];
    let w = waves(&plans).expect("a valid order exists");
    let order: Vec<&str> = w.iter().flatten().map(|p| p.item_id.as_str()).collect();
    assert_eq!(order, vec!["port", "adapter", "facade"]);
    assert_eq!(w.len(), 3, "a dependency chain was collapsed into one wave");
}

/// A plan that changes nothing cannot be scheduled against another, checked
/// for overlap, or shown to have been carried out.
#[test]
fn a_plan_that_writes_nothing_is_refused() {
    let plans = vec![plan("empty", &[], &[])];
    match waves(&plans) {
        Err(Refusal::NothingToWrite { item_id }) => assert_eq!(item_id, "empty"),
        other => panic!("a plan naming no write set was accepted: {other:?}"),
    }
}

/// No order exists, and saying so beats emitting one that cannot land.
#[test]
fn a_dependency_loop_is_refused_and_names_its_members() {
    let plans = vec![
        plan("a", &["src/a.rs"], &["b"]),
        plan("b", &["src/b.rs"], &["a"]),
        plan("free", &["src/free.rs"], &[]),
    ];
    match waves(&plans) {
        Err(Refusal::NoValidOrder { members }) => {
            assert_eq!(members, vec!["a".to_string(), "b".to_string()]);
            assert!(
                !members.contains(&"free".to_string()),
                "an unrelated plan was dragged into the loop"
            );
        }
        other => panic!("a dependency loop was accepted: {other:?}"),
    }
}

#[test]
fn a_dependency_on_something_absent_is_refused() {
    let plans = vec![plan("a", &["src/a.rs"], &["never-planned"])];
    match waves(&plans) {
        Err(Refusal::DependsOnSomethingAbsent { item_id, missing }) => {
            assert_eq!(item_id, "a");
            assert_eq!(missing, "never-planned");
        }
        other => panic!("a dangling dependency was accepted: {other:?}"),
    }
}

/// Every refusal must say what to do. One that cannot is a finding nobody can
/// act on — the defect this codebase gates against elsewhere.
#[test]
fn every_refusal_states_a_remedy() {
    let cases = vec![
        Refusal::NothingToWrite {
            item_id: "x".into(),
        },
        Refusal::OverlappingWriteSets {
            a: "x".into(),
            b: "y".into(),
            paths: vec!["src/z.rs".into()],
        },
        Refusal::NoValidOrder {
            members: vec!["x".into()],
        },
        Refusal::DependsOnSomethingAbsent {
            item_id: "x".into(),
            missing: "y".into(),
        },
    ];
    for c in cases {
        let r = c.remedy();
        assert!(r.len() > 40, "refusal {c:?} gives no usable remedy: {r}");
    }
}

/// Sequencing hides a conflict; naming it lets the write sets be changed while
/// that is still cheap.
#[test]
fn conflicts_are_reported_separately_from_sequencing() {
    let plans = vec![
        plan("a", &["src/shared.rs"], &[]),
        plan("b", &["src/shared.rs"], &[]),
        plan("c", &["src/c.rs"], &[]),
    ];
    let found = conflicts(&plans);
    assert_eq!(found.len(), 1, "the one real conflict was not named");
    match &found[0] {
        Refusal::OverlappingWriteSets { a, b, paths } => {
            assert_eq!((a.as_str(), b.as_str()), ("a", "b"));
            assert_eq!(paths, &vec!["src/shared.rs".to_string()]);
        }
        other => panic!("wrong refusal: {other:?}"),
    }
    assert!(waves(&plans).is_ok(), "a sequencable conflict was refused");
}

#[test]
fn no_plans_is_no_waves_rather_than_an_error() {
    assert_eq!(waves(&[]).expect("nothing to schedule").len(), 0);
}

#[test]
fn two_plans_touching_different_hub_files_do_not_share_a_wave() {
    // Disjoint write-sets, so plain set intersection admits both. The hub rule
    // does not: `Cargo.toml` and `src/lib.rs` are both files every lane must
    // edit, and two hub hops in flight together is how this repository's own
    // branches collided on `src/lib.rs` and `src/migration/registry.rs`.
    let plans = vec![
        plan("a", &["Cargo.toml"], &[]),
        plan("b", &["src/lib.rs"], &[]),
    ];
    let w = waves(&plans).expect("schedulable");
    assert_eq!(
        w.len(),
        2,
        "each hub hop goes alone; got {:?}",
        w.iter()
            .map(|wave| wave.iter().map(|p| p.item_id.clone()).collect::<Vec<_>>())
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_hub_plan_and_an_ordinary_plan_still_share_a_wave() {
    // The hub rule bounds hub hops against each other, not against everything.
    // A rule that serialised the whole wave behind one hub write would be a
    // crate lock by another name, which D-39 rejects.
    let plans = vec![
        plan("hub", &["src/lib.rs"], &[]),
        plan("leaf", &["src/widget/mod.rs"], &[]),
    ];
    let w = waves(&plans).expect("schedulable");
    assert_eq!(w.len(), 1, "one hub plus disjoint leaves is one wave");
    assert_eq!(w[0].len(), 2);
}

#[test]
fn the_disjointness_decision_is_not_reimplemented_here() {
    // `plan` must not carry its own copy of the occupancy predicate. The first
    // version did, and lost the hub rule with it.
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/plan/mod.rs"))
        .expect("plan source");
    assert!(
        src.contains("admit_spawn"),
        "waves must delegate the admission decision to change_delivery"
    );
}
