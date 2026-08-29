//! Global shape of the unit dependency graph: cycles, depth, and fan-in.
//!
//! `dependency.rs` enforces the *direction* of an edge — which face may name
//! which, and that only a facade is reachable across units. That is a local
//! question, asked one edge at a time. It cannot see the properties that only
//! exist for the graph as a whole:
//!
//!   * **Cycles.** Rust forbids a dependency cycle between crates, so a cycle
//!     among units is the thing that makes those units unsplittable. Anvil's
//!     own tree holds one strongly-connected component of 70 modules; oyatie
//!     holds one cycle among 791 crates. Neither is visible to a per-edge rule
//!     — every individual edge in a cycle is legal.
//!   * **Depth.** The longest chain bounds how many sequential waves any plan
//!     needs, whatever the agent count.
//!   * **Fan-in.** How many units a change to one unit obliges you to retest,
//!     and how many are gated when it sits in an open pull request.
//!
//! Measured, not asserted: these three numbers were computed by hand over
//! oyatie's 791 crates — depth 9, worst fan-in 173
//! (22% of the repo), one cycle. Hand-computing them four times is what this
//! module exists to stop.

use std::collections::{BTreeMap, BTreeSet};

use super::dependency::{DepGraph, classify};
use super::report::{Finding, RuleId};
use super::resolve::{ResolvedSpec, ResolvedUnit};

/// Edges between units, with intra-unit edges dropped.
///
/// A unit depending on itself is not a cycle, it is a unit.
pub fn unit_graph(
    spec: &ResolvedSpec,
    units: &[ResolvedUnit],
    graph: &DepGraph,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for e in &graph.edges {
        let (Some((from, _)), Some((to, _))) =
            (classify(spec, units, &e.from), classify(spec, units, &e.to))
        else {
            continue;
        };
        if from.name != to.name {
            out.entry(from.name.clone())
                .or_default()
                .insert(to.name.clone());
        }
    }
    out
}

/// Strongly-connected components with more than one member, each sorted.
///
/// Tarjan, iterative: a recursive walk overflows on a 70-member component,
/// and the component this exists to find is exactly that size.
pub fn cycles(adj: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    let nodes: BTreeSet<&String> = adj.keys().chain(adj.values().flatten()).collect();
    let mut index = BTreeMap::new();
    let mut low = BTreeMap::new();
    let mut on_stack = BTreeSet::new();
    let mut stack: Vec<&String> = Vec::new();
    let mut out = Vec::new();
    let mut counter = 0usize;
    let empty = BTreeSet::new();

    for root in nodes {
        if index.contains_key(root) {
            continue;
        }
        // (node, next child to visit)
        let mut work: Vec<(&String, usize)> = vec![(root, 0)];
        while let Some(&(v, pi)) = work.last() {
            if pi == 0 {
                index.insert(v, counter);
                low.insert(v, counter);
                counter += 1;
                stack.push(v);
                on_stack.insert(v);
            }
            let children: Vec<&String> = adj.get(v).unwrap_or(&empty).iter().collect();
            let mut descended = false;
            for (i, w) in children.iter().enumerate().skip(pi) {
                if !index.contains_key(*w) {
                    work.last_mut().expect("non-empty").1 = i + 1;
                    work.push((w, 0));
                    descended = true;
                    break;
                } else if on_stack.contains(*w) {
                    let lv = low[v].min(index[*w]);
                    low.insert(v, lv);
                }
            }
            if descended {
                continue;
            }
            work.pop();
            if let Some(&(parent, _)) = work.last() {
                let lp = low[parent].min(low[v]);
                low.insert(parent, lp);
            }
            if low[v] == index[v] {
                let mut comp = Vec::new();
                while let Some(w) = stack.pop() {
                    on_stack.remove(w);
                    comp.push(w.clone());
                    if w == v {
                        break;
                    }
                }
                if comp.len() > 1 {
                    comp.sort();
                    out.push(comp);
                }
            }
        }
    }
    out.sort();
    out
}

/// One finding per cycle. The key is the member list, so a cycle that gains
/// or loses a member is a different finding rather than a silent mutation of
/// the same one.
pub fn cycle_findings(
    spec: &ResolvedSpec,
    units: &[ResolvedUnit],
    graph: &DepGraph,
) -> Vec<Finding> {
    cycles(&unit_graph(spec, units, graph))
        .into_iter()
        .map(|comp| Finding {
            rule: RuleId::new("unit_dependency_cycle"),
            key: comp.join(","),
            path: String::new(),
            unit: comp.first().cloned(),
            detail: format!(
                "{} units form a dependency cycle: {}. Every edge in it is \
                 individually legal, which is why no per-edge rule can see it. \
                 Rust forbids a cycle between crates, so these units cannot be \
                 split while it stands.",
                comp.len(),
                comp.join(" -> ")
            ),
            fix: None,
        })
        .collect()
}
