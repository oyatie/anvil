//! Ordering the queue, from facts already on the items.
//!
//! # The rule this obeys
//!
//! Every element of the order is read off the item. Nothing here weighs,
//! scores or estimates: a number invented by this module would be a judgement
//! wearing the costume of a measurement, and the whole point of the queue is
//! that what it says can be checked.
//!
//! # Unclassified is not low priority
//!
//! An item whose remedy nobody has decided cannot be placed honestly. Sorting
//! it into the middle asserts a position that was never determined; sorting it
//! to the bottom is worse, because "nobody has looked at this" then behaves
//! exactly like "this does not matter". They are listed separately, and the
//! count of them is the thing to act on — it says how much of the backlog is
//! not yet knowable rather than pretending it is ordered.
//!
//! # Recurrence outranks novelty
//!
//! A class seen three times is not three problems; it is one rule that should
//! have been written after the first. Recurrence is therefore part of the
//! order, ahead of anything singular at the same urgency — which is the
//! doctrine that a repeated defect is evidence of a missing mechanism, made
//! operational rather than left as advice.

use std::collections::BTreeMap;

use super::{Queue, Remedy, Source, WorkItem};

/// Why an item is urgent, derived from what raised it.
///
/// Ordered most urgent first. Each variant is a fact about the source, not an
/// opinion about the work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    /// Something is broken now.
    Live,
    /// A published vulnerability.
    Security,
    /// Declared and actual have diverged.
    Drifted,
    /// Everything else: real, and not on fire.
    Standing,
}

impl Urgency {
    /// Read off the source. No item is assigned an urgency it did not arrive
    /// with.
    pub fn of(item: &WorkItem) -> Self {
        match item.source {
            Source::Incident => Urgency::Live,
            Source::Advisory => Urgency::Security,
            Source::Drift | Source::Regression => Urgency::Drifted,
            Source::Direction
            | Source::ReviewFinding
            | Source::Audit
            | Source::PostmortemRemedy => Urgency::Standing,
        }
    }
}

/// One item, with the facts the order is built from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triaged<'a> {
    pub item: &'a WorkItem,
    pub urgency: Urgency,
    /// How many items in this queue share this item's class. One when the
    /// class is unique, and one when the item names no class at all — an
    /// unnamed class cannot be counted, and must not be scored as if it were
    /// rare.
    pub recurrence: usize,
    /// Whether a machine can close it. Cheap-and-certain goes before
    /// expensive-and-uncertain at the same urgency and recurrence.
    pub mechanical: bool,
}

/// The queue, ordered — and the part of it that cannot be ordered yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triage<'a> {
    /// Most urgent first; within an urgency, most-recurring first; within
    /// that, mechanical before judgement.
    pub ordered: Vec<Triaged<'a>>,
    /// Items whose remedy nobody has decided. Not ranked, because there is
    /// nothing yet to rank them by.
    pub unclassified: Vec<&'a WorkItem>,
}

impl Triage<'_> {
    /// How much of the backlog is not yet knowable.
    pub fn unclassified_share(&self) -> f64 {
        let total = self.ordered.len() + self.unclassified.len();
        if total == 0 {
            return 0.0;
        }
        self.unclassified.len() as f64 / total as f64
    }

    /// Classes appearing more than once, most frequent first. Each is a rule
    /// that should already exist.
    pub fn recurring_classes(&self) -> Vec<(&str, usize)> {
        let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
        for t in &self.ordered {
            if let Some(c) = t.item.class.as_deref() {
                *counts.entry(c).or_default() += 1;
            }
        }
        let mut out: Vec<(&str, usize)> = counts.into_iter().filter(|(_, n)| *n > 1).collect();
        out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
        out
    }
}

/// Order the queue by what its items already say about themselves.
pub fn triage(queue: &Queue) -> Triage<'_> {
    let by_class = queue.by_class();
    let mut ordered: Vec<Triaged> = Vec::new();
    let mut unclassified: Vec<&WorkItem> = Vec::new();

    for item in queue.outstanding() {
        if matches!(item.remedy, Remedy::Unclassified) {
            unclassified.push(item);
            continue;
        }
        ordered.push(Triaged {
            item,
            urgency: Urgency::of(item),
            recurrence: item
                .class
                .as_deref()
                .and_then(|c| by_class.get(c).copied())
                .unwrap_or(1),
            mechanical: matches!(item.remedy, Remedy::Mechanical { .. }),
        });
    }

    ordered.sort_by(|a, b| {
        a.urgency
            .cmp(&b.urgency)
            .then(b.recurrence.cmp(&a.recurrence))
            .then(b.mechanical.cmp(&a.mechanical))
            // Ties break on identity so the order is stable across runs; an
            // order that shuffles is one nobody can work through.
            .then(a.item.id().cmp(&b.item.id()))
    });

    Triage {
        ordered,
        unclassified,
    }
}
