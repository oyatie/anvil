//! Cycles among units, seeded and spared.
//!
//! Every case here is one the per-edge dependency rules cannot see: each
//! individual edge in a cycle is legal, so a rule that judges one edge at a
//! time reports a clean tree while the units remain unsplittable.

use std::collections::{BTreeMap, BTreeSet};

use anvil::shape::core::graph_shape::cycles;

fn g(pairs: &[(&str, &str)]) -> BTreeMap<String, BTreeSet<String>> {
    let mut m: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (a, b) in pairs {
        m.entry((*a).into()).or_default().insert((*b).into());
    }
    m
}

#[test]
fn an_acyclic_graph_has_no_cycles() {
    assert!(cycles(&g(&[])).is_empty(), "empty graph");
    assert!(cycles(&g(&[("a", "b"), ("b", "c")])).is_empty(), "a chain");
    assert!(
        cycles(&g(&[("a", "b"), ("a", "c"), ("b", "d"), ("c", "d")])).is_empty(),
        "a diamond is not a cycle: both paths run the same direction"
    );
}

#[test]
fn a_two_unit_cycle_is_found() {
    assert_eq!(cycles(&g(&[("a", "b"), ("b", "a")])), vec![vec!["a", "b"]]);
}

#[test]
fn a_longer_cycle_is_found_whole_not_pairwise() {
    assert_eq!(
        cycles(&g(&[("a", "b"), ("b", "c"), ("c", "a")])),
        vec![vec!["a", "b", "c"]],
        "a three-unit cycle is one finding, not three"
    );
}

#[test]
fn two_disjoint_cycles_are_two_findings() {
    let found = cycles(&g(&[
        ("a", "b"),
        ("b", "a"),
        ("x", "y"),
        ("y", "z"),
        ("z", "x"),
    ]));
    assert_eq!(found, vec![vec!["a", "b"], vec!["x", "y", "z"]]);
}

/// A cycle hanging off an acyclic spine must not drag the spine in with it.
#[test]
fn only_the_cycle_is_reported_not_its_neighbours() {
    let found = cycles(&g(&[
        ("root", "a"),
        ("a", "b"),
        ("b", "a"),
        ("b", "leaf"),
    ]));
    assert_eq!(found, vec![vec!["a", "b"]], "root and leaf are acyclic");
}

/// Anvil's own tree holds a strongly-connected component of seventy modules.
/// A recursive Tarjan overflows the stack on exactly the input this exists to
/// find, so the walk is iterative and this is the case that proves it.
#[test]
fn a_seventy_member_cycle_is_found_without_overflowing() {
    let names: Vec<String> = (0..70).map(|i| format!("m{i:02}")).collect();
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    for i in 0..70 {
        pairs.push((names[i].as_str(), names[(i + 1) % 70].as_str()));
    }
    let found = cycles(&g(&pairs));
    assert_eq!(found.len(), 1, "one component, not seventy");
    assert_eq!(found[0].len(), 70);
}

/// The rule must not fire on a graph that merely has many edges.
#[test]
fn a_wide_acyclic_graph_is_spared() {
    let hub: Vec<(String, String)> = (0..200).map(|i| (format!("u{i}"), "hub".to_string())).collect();
    let pairs: Vec<(&str, &str)> = hub.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    assert!(
        cycles(&g(&pairs)).is_empty(),
        "200 units depending on one hub is high fan-in, not a cycle"
    );
}
