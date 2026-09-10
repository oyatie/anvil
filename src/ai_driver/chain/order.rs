//! Reading the order the routing table declares.
//!
//! Kept out of `chain.rs` for the same reason the loader and the scope writer
//! are: deciding which model serves a stage and walking a graph between stages
//! are different jobs.

use super::{Stage, StagePlan, plan};
use anyhow::{Result, bail};
use std::collections::BTreeMap;

/// Whether `later` must run after `earlier`, following the declared order
/// through as many hops as it takes.
///
/// The direct question is almost never the useful one: `implementation` does
/// not name `test_authoring`, it names `test_authoring_review`, which audits
/// `test_authoring`. A caller that asked only about direct edges would conclude
/// the implementer may run first.
///
/// The loader refuses a cyclic table, so this terminates.
#[must_use]
pub fn runs_after_transitively(later: Stage, earlier: Stage) -> bool {
    let mut seen = BTreeMap::new();
    let mut frontier = vec![later];
    while let Some(stage) = frontier.pop() {
        if seen.insert(stage, ()).is_some() {
            continue;
        }
        let p = plan(stage);
        for key in p.runs_after.iter().chain(p.audits.iter()) {
            let Some(next) = Stage::ALL.iter().find(|s| s.key() == key) else {
                continue;
            };
            if *next == earlier {
                return true;
            }
            frontier.push(*next);
        }
    }
    false
}

/// Every ordering edge the file declares, explicit and derived.
///
/// `audits = X` IS an ordering claim -- you cannot judge what has not run -- so
/// the edge is derived rather than restated. `test_audit` declared both, which
/// is what made the redundancy visible.
fn order_edges(plan: &StagePlan) -> impl Iterator<Item = &String> {
    plan.runs_after.iter().chain(plan.audits.iter())
}

/// The declared order must name real stages, must not restate what `audits`
/// already implies, and must admit an order at all.
///
/// A cycle is not a stylistic complaint: it means no sequence satisfies the
/// file, so any runner walking it either loops or silently picks one edge to
/// ignore. Refusing at load is the only point where that is still cheap.
pub(super) fn validate(out: &BTreeMap<Stage, StagePlan>) -> Result<()> {
    let by_key: BTreeMap<&str, Stage> = out.keys().map(|s| (s.key(), *s)).collect();

    for (stage, plan) in out {
        for named in &plan.runs_after {
            if !by_key.contains_key(named.as_str()) {
                bail!(
                    "stage `{}` declares it runs after `{named}`, which no `Stage` names. \
                     An order over a stage that does not exist is not an order.",
                    stage.key()
                );
            }
            if plan.runs_after.iter().filter(|k| *k == named).count() > 1 {
                bail!(
                    "stage `{}` lists `{named}` in `runs_after` more than once. Kahn's \
                     in-degree counts the entries and the decrement fires once per \
                     predecessor, so a repeat would be reported as a CYCLE -- a diagnostic \
                     naming the wrong defect is worse than none.",
                    stage.key()
                );
            }
            if plan.audits.as_deref() == Some(named.as_str()) {
                bail!(
                    "stage `{}` both audits `{named}` and lists it in `runs_after`. Auditing \
                     it already means running after it; two spellings of one relation drift \
                     apart, which is how `authored_before` and `runs_after` came to disagree.",
                    stage.key()
                );
            }
        }
    }

    // Kahn's algorithm. What is left when no node has zero remaining
    // predecessors is exactly the cycle, and it is named rather than summarised
    // -- a diagnostic that says "there is a cycle" leaves the reader to find it.
    let mut pending: BTreeMap<Stage, usize> = out
        .iter()
        .map(|(s, p)| (*s, order_edges(p).count()))
        .collect();
    let mut ready: Vec<Stage> = pending
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(s, _)| *s)
        .collect();

    let mut settled = 0usize;
    while let Some(done) = ready.pop() {
        settled += 1;
        pending.remove(&done);
        for (stage, plan) in out {
            if !pending.contains_key(stage) {
                continue;
            }
            if order_edges(plan).any(|k| k.as_str() == done.key()) {
                let left = pending.get_mut(stage).expect("still pending");
                *left -= 1;
                if *left == 0 {
                    ready.push(*stage);
                }
            }
        }
    }

    if settled != out.len() {
        let stuck: Vec<&str> = pending.keys().map(|s| s.key()).collect();
        bail!(
            "the declared order has a cycle among {stuck:?}, so no sequence satisfies \
             config/model-routing.toml. A runner walking it would loop, or would drop one \
             edge without saying which."
        );
    }
    Ok(())
}
