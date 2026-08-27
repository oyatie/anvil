//! One shape for every intent, and one queue to hold them.
//!
//! # Why this exists
//!
//! Seven things raise work in this codebase — the issue reconciler, the
//! roadmap guard, the incident sentry, the zero-day patcher, the GitOps drift
//! reconciler, the corpus auditor, the review memory — and each raises its own
//! shape. `ReconciledIssue` carries a GitHub issue number; `GitOpsDriftReport`
//! carries orphan manifests; `ZeroDayReport` carries advisories. Nothing can
//! compare across them, prioritise between them, or answer what is
//! outstanding.
//!
//! The consequence is worse than untidiness. The continuous audits — whole-repo
//! conformance, drift, the postmortem ledger's own unbuilt remedies — produce
//! knowledge that re-enters nothing, so the loop from LEARN back to INTAKE is
//! an arc. A finding printed and not queued is a finding that will be found
//! again.
//!
//! # Identity is derived, never generated
//!
//! The property that makes a queue a queue rather than a log: raising the same
//! finding twice must yield ONE item. Every audit pass re-reports everything it
//! can still see, so a generated id would grow the backlog linearly with the
//! number of sweeps and never converge. The id is a function of what the
//! finding IS — source, subject, and the finding itself — so re-raising is
//! idempotent by construction rather than by a de-duplication pass somebody has
//! to remember to run.
//!
//! This exact defect is already recorded elsewhere in this repository: the
//! recovery sweep re-certified every open pull request on every pass because
//! nothing recorded what it had already done.

pub mod sources;
pub mod triage;

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// What raised this item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// A person asked for it.
    Direction,
    /// A review said so.
    ReviewFinding,
    /// A gate or sweep found it.
    Audit,
    /// Something broke.
    Incident,
    /// A postmortem named a remedy that does not exist yet.
    PostmortemRemedy,
    /// A dependency advisory.
    Advisory,
    /// Declared and actual drifted apart.
    Drift,
    /// A ratchet moved the wrong way.
    Regression,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::Direction => "direction",
            Source::ReviewFinding => "review",
            Source::Audit => "audit",
            Source::Incident => "incident",
            Source::PostmortemRemedy => "postmortem",
            Source::Advisory => "advisory",
            Source::Drift => "drift",
            Source::Regression => "regression",
        }
    }
}

/// What the item is about. Absent when it is about the repository itself.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Subject {
    pub repo: String,
    /// A path, a gate id, a crate — whatever names the thing precisely.
    pub locus: Option<String>,
}

/// Whether a machine can do this, and if not, why not.
///
/// The distinction is the whole point of recording it. A class defended only
/// by judgement will recur, because the next instance needs someone to notice
/// again; `why_judgement` forces that to be argued rather than assumed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Remedy {
    /// A codemod or a rule can close it. Names what to run.
    Mechanical { how: String },
    /// It needs a person, and here is why a machine cannot.
    NeedsJudgement { why: String },
    /// Not yet decided. Distinct from "needs judgement": nobody has looked.
    Unclassified,
}

/// One piece of work, whatever raised it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkItem {
    pub source: Source,
    pub subject: Subject,
    /// The finding, in the terms someone would recognise it by.
    pub what: String,
    /// What is lost while it stands. An item that cannot say this is one
    /// nobody can prioritise.
    pub consequence: String,
    /// The `postmortem::FixClass` id, when this is a known class. Recurrence
    /// is countable only if instances name their class.
    pub class: Option<String>,
    pub remedy: Remedy,
}

impl WorkItem {
    /// Stable identity, derived from the finding rather than generated.
    ///
    /// Deliberately excludes `remedy` and `consequence`: re-classifying an
    /// item or describing its cost better must not create a second one.
    pub fn id(&self) -> String {
        let mut s = String::new();
        let _ = write!(
            s,
            "{}:{}:{}:{}",
            self.source.as_str(),
            self.subject.repo,
            self.subject.locus.as_deref().unwrap_or("-"),
            self.what
        );
        s
    }
}

/// Everything outstanding, keyed by derived identity.
#[derive(Debug, Clone, Default)]
pub struct Queue {
    items: BTreeMap<String, WorkItem>,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item, or update the one already standing for it.
    ///
    /// Returns whether this was new. Re-raising an existing finding updates it
    /// in place -- a better-classified remedy should replace a worse one --
    /// without adding a second entry.
    pub fn raise(&mut self, item: WorkItem) -> bool {
        self.items.insert(item.id(), item).is_none()
    }

    /// Everything outstanding, in a stable order.
    pub fn outstanding(&self) -> Vec<&WorkItem> {
        self.items.values().collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// How many items name each class, for the classes that name one.
    ///
    /// Recurrence is the signal that a deterministic rule is owed: a class
    /// seen twice was a rule that should have been written after the first.
    pub fn by_class(&self) -> BTreeMap<&str, usize> {
        let mut out: BTreeMap<&str, usize> = BTreeMap::new();
        for item in self.items.values() {
            if let Some(c) = item.class.as_deref() {
                *out.entry(c).or_default() += 1;
            }
        }
        out
    }

    /// Items nobody has decided are mechanical or not.
    pub fn unclassified(&self) -> Vec<&WorkItem> {
        self.items
            .values()
            .filter(|i| matches!(i.remedy, Remedy::Unclassified))
            .collect()
    }
}
