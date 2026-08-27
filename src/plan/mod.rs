//! What a work item proposes to do, declared before it is done.
//!
//! # Why a plan is a value rather than a description
//!
//! A plan's worth is that it can be REFUSED before anyone writes a line. Every
//! refusal here costs nothing at this point and costs a great deal later: two
//! lanes given overlapping write-sets become an N-squared consolidation, and a
//! sequence with no valid order becomes a stack of pull requests that cannot
//! land in any sequence.
//!
//! Nothing here executes, fetches or edits. A plan is a declaration, and the
//! checks are questions the declaration can be asked.
//!
//! # What is deliberately not decided here
//!
//! Whether an added dependency edge would close a cycle in the CRATE graph is
//! the other half of this, and it lives with the graph. That check is on an
//! unmerged branch; depending on it would stack this change on that one, which
//! is the diamond this module exists to help avoid. `adds_edges` is declared
//! and carried so the check has something to read when it arrives.
//!
//! # The order of waves is over PLANS, not crates
//!
//! A plan can depend on another plan — a port before its adapters, a
//! vocabulary before its consumers. That graph is small, self-contained, and
//! must be acyclic or no order exists at all. It is not the crate dependency
//! graph and must not be confused with it.

use std::collections::{BTreeMap, BTreeSet};

/// What a work item proposes to change.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Plan {
    /// The work item this plan is for, by its derived identity.
    pub item_id: String,
    /// Paths this plan will write. The unit of lane disjointness.
    ///
    /// Empty is refused rather than treated as harmless: a plan that names
    /// nothing cannot be scheduled against another, cannot be checked for
    /// overlap, and cannot be shown to have been carried out.
    pub write_set: BTreeSet<String>,
    /// Dependency edges this plan would ADD, as `(from, to)` unit names.
    ///
    /// Carried for the cycle check that lives with the graph. Declaring an
    /// edge before it exists is what makes refusing it free.
    pub adds_edges: BTreeSet<(String, String)>,
    /// Plans that must land first, by `item_id`.
    pub depends_on: BTreeSet<String>,
}

/// Why a set of plans cannot be scheduled as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A plan that changes nothing cannot be scheduled or verified.
    NothingToWrite { item_id: String },
    /// Two plans in the same wave would write the same path.
    OverlappingWriteSets {
        a: String,
        b: String,
        paths: Vec<String>,
    },
    /// The plans depend on each other in a loop, so no order exists.
    NoValidOrder { members: Vec<String> },
    /// A plan depends on something that is not among the plans given.
    DependsOnSomethingAbsent { item_id: String, missing: String },
}

impl Refusal {
    /// What a reader should do about it. A refusal that cannot say this is one
    /// nobody can act on.
    pub fn remedy(&self) -> String {
        match self {
            Refusal::NothingToWrite { item_id } => format!(
                "`{item_id}` declares no write set. Name the paths it will \
                 change, or drop it: a plan that changes nothing cannot be \
                 shown to have been carried out."
            ),
            Refusal::OverlappingWriteSets { a, b, paths } => format!(
                "`{a}` and `{b}` both write {paths:?}. Give one of them a \
                 different write set, or sequence them — two lanes on one path \
                 is the conflict that makes consolidation quadratic."
            ),
            Refusal::NoValidOrder { members } => format!(
                "{members:?} depend on each other in a loop, so no order lands \
                 them. Split one plan, or drop a dependency."
            ),
            Refusal::DependsOnSomethingAbsent { item_id, missing } => format!(
                "`{item_id}` depends on `{missing}`, which is not among these \
                 plans. Include it, or the wave that needs it never becomes \
                 ready."
            ),
        }
    }
}

/// Plans grouped into waves: everything in a wave may run at once.
pub type Waves<'a> = Vec<Vec<&'a Plan>>;

/// Group plans into waves, or refuse and say why.
///
/// A wave is a set of plans whose dependencies have all landed in earlier
/// waves AND whose write sets are pairwise disjoint. Both conditions are
/// necessary: dependencies alone give an order that still conflicts, and
/// disjointness alone gives lanes that cannot build.
pub fn waves(plans: &[Plan]) -> Result<Waves<'_>, Refusal> {
    for p in plans {
        if p.write_set.is_empty() {
            return Err(Refusal::NothingToWrite {
                item_id: p.item_id.clone(),
            });
        }
    }
    let known: BTreeSet<&str> = plans.iter().map(|p| p.item_id.as_str()).collect();
    for p in plans {
        for d in &p.depends_on {
            if !known.contains(d.as_str()) {
                return Err(Refusal::DependsOnSomethingAbsent {
                    item_id: p.item_id.clone(),
                    missing: d.clone(),
                });
            }
        }
    }

    let by_id: BTreeMap<&str, &Plan> = plans.iter().map(|p| (p.item_id.as_str(), p)).collect();
    let mut landed: BTreeSet<&str> = BTreeSet::new();
    let mut out: Waves = Vec::new();

    while landed.len() < plans.len() {
        // Ready: every dependency already landed.
        let ready: Vec<&Plan> = plans
            .iter()
            .filter(|p| !landed.contains(p.item_id.as_str()))
            .filter(|p| p.depends_on.iter().all(|d| landed.contains(d.as_str())))
            .collect();

        if ready.is_empty() {
            // Nothing can start and work remains: the remainder is a loop.
            let mut members: Vec<String> = plans
                .iter()
                .filter(|p| !landed.contains(p.item_id.as_str()))
                .map(|p| p.item_id.clone())
                .collect();
            members.sort();
            return Err(Refusal::NoValidOrder { members });
        }

        // Within the wave, admit only plans that do not collide. A plan held
        // back is not refused -- it simply waits for the next wave, which is
        // the difference between sequencing and rejecting.
        let mut wave: Vec<&Plan> = Vec::new();
        let mut claimed: BTreeSet<&str> = BTreeSet::new();
        for p in ready {
            let overlap: Vec<&str> = p
                .write_set
                .iter()
                .map(String::as_str)
                .filter(|path| claimed.contains(path))
                .collect();
            if overlap.is_empty() {
                for path in &p.write_set {
                    claimed.insert(path.as_str());
                }
                wave.push(p);
            }
        }
        for p in &wave {
            landed.insert(p.item_id.as_str());
        }
        out.push(wave);
    }
    let _ = by_id;
    Ok(out)
}

/// Every pair of plans that would write the same path, whatever the ordering.
///
/// Reported separately from `waves`, which merely sequences around a conflict.
/// Two plans that can never share a wave are worth knowing about while the
/// write sets can still be changed.
pub fn conflicts(plans: &[Plan]) -> Vec<Refusal> {
    let mut out = Vec::new();
    for (i, a) in plans.iter().enumerate() {
        for b in plans.iter().skip(i + 1) {
            let shared: Vec<String> = a.write_set.intersection(&b.write_set).cloned().collect();
            if !shared.is_empty() {
                out.push(Refusal::OverlappingWriteSets {
                    a: a.item_id.clone(),
                    b: b.item_id.clone(),
                    paths: shared,
                });
            }
        }
    }
    out
}
