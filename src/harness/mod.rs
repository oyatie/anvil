//! One harness, many rules. Adding a rule is a registration, not a new gate.
//!
//! # What this replaces
//!
//! Governance checks were written one at a time: a module, a test file, a CI
//! job and a bespoke report shape each. Fifteen rules had accumulated four
//! different notions of "no finding" and no shared notion of "could not run".
//!
//! The failure that motivated this appeared seven times in one day, always
//! wearing a different costume, always the same defect: **absence of a finding
//! read as absence of a problem**.
//!
//!   * a gate status where `NotMeasured.is_acceptable()` was `true`, so 72
//!     unmeasured gates published as "72 passed, 0 failed"
//!   * `not_measured` carried by the core report and dropped by the facade, so
//!     a run where nothing executed returned `Passed`
//!   * eleven of fifteen rules undeclared in the spec, producing no finding AND
//!     no unmeasured entry -- invisible rather than absent
//!   * three bare `continue`s discarding inputs nobody could classify
//!   * a test guard counting `FAILED` lines, where a tree that did not compile
//!     printed zero of them and read as success
//!
//! Each was fixed individually. That is the N+1 trap: seven fixes, one class,
//! and an eighth instance a keystroke away.
//!
//! # How the class dies
//!
//! A rule cannot return a bare list. [`Evaluated`] has no variant meaning
//! "nothing to report" that is reachable without also proving coverage:
//! `Measured` requires a [`NonZeroUsize`] of subjects examined, so a
//! measurement over nothing is unconstructible, and `Withheld` is a different
//! variant that no consumer can mistake for clean.
//!
//! # Proven before trusted
//!
//! Cheap and early is worth nothing if the check silently passes -- an
//! unproven early check is worse than none, because it occupies the slot and
//! buys down scrutiny downstream without earning it. Eleven inert rules were
//! strictly worse than eleven absent ones.
//!
//! So [`Rule::fixture`] is not optional. Every rule ships a subject it MUST
//! flag and a subject it MUST NOT, and [`Harness::run`] executes both before
//! trusting any verdict. A rule that cannot demonstrate it fires is withheld,
//! not believed.

use std::collections::BTreeMap;
use std::num::NonZeroUsize;

pub mod apply;
pub mod corpus;
pub mod judgement;
pub mod rules;

pub use apply::{Edit, Plan, Refused, plan};
pub use corpus::{Corpus, Subject};

/// What a rule needs in order to run, and therefore the cheapest stage that can
/// host it.
///
/// A defect caught late pays the sunk cost of everything that carried it there,
/// plus re-traversal of every stage below. Eleven of fifteen shipped rules need
/// only paths and were running in the certification pipeline: a misnamed crate
/// cost a full CI cycle and a merge-queue slot to discover, when it could have
/// cost a red underline.
///
/// Declared per rule so the harness places it, rather than each rule choosing.
/// A rule that declares `PathsOnly` and reads a file is lying about its inputs,
/// which is checkable because the corpus records what was accessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Requires {
    /// Editor, pre-commit. Paths and nothing else.
    PathsOnly,
    /// Pre-commit. File contents, no build.
    FileContents,
    /// Pre-commit. The change under review: two revisions and the patch text
    /// between them.
    ///
    /// This rung is why roughly half the shipped gates could not be expressed
    /// here at all. A corpus of the working tree answers "is this file wrong";
    /// most of the pre-merge gates ask "does this change make it wrong", which
    /// is a question about a pair of revisions and cannot be asked of one.
    Changeset,
    /// Pre-push. Cargo manifests.
    Manifests,
    /// Pre-push. The commit subjects the change adds, as `git log base..head`
    /// reports them.
    ///
    /// Separate from [`Requires::Changeset`] because the range can be present
    /// while the log is not, and a gate that read an absent log as an empty one
    /// published an accusation at every pull request whose commits never
    /// reached it.
    History,
    /// Presubmit. The resolved dependency graph.
    BuildGraph,
    /// Presubmit. A toolchain the rule may invoke -- cargo, clippy, buck2.
    Toolchain,
    /// Merge queue. Remote state: pull request status, checks, registries.
    ///
    /// The most expensive rung and the only one that can fail for reasons
    /// unrelated to the change. A rule here must be withheld on a network
    /// error, never passed.
    Network,
}

impl Requires {
    /// Human name of the cheapest stage that can host this rule.
    pub fn stage(self) -> &'static str {
        match self {
            Requires::PathsOnly => "editor",
            Requires::FileContents | Requires::Changeset => "pre-commit",
            Requires::Manifests | Requires::History => "pre-push",
            Requires::BuildGraph | Requires::Toolchain => "presubmit",
            Requires::Network => "merge-queue",
        }
    }
}

/// Why a rule could not run.
///
/// A variant per cause rather than a string, so a consumer can branch on the
/// difference between "the spec never asked" and "the inputs were missing".
/// Both are withheld; only one is a configuration error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Withheld {
    /// The engine knows this rule; the spec never declared it.
    Undeclared,
    /// The corpus lacks what [`Rule::requires`] asked for.
    InputsAbsent { needed: Requires },
    /// The rule's own fixture stopped behaving, so its verdicts are not trusted.
    FixtureFailed { detail: String },
    /// The rule ran and could not classify some input. Named rather than
    /// dropped: an input nobody could classify is not an input that passed.
    Unclassifiable { subjects: Vec<String> },
}

/// The outcome of running one rule. There is deliberately no third variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evaluated {
    /// Ran over at least one subject. Empty `findings` means clean, and the
    /// `NonZeroUsize` is what makes that claim honest.
    Measured {
        subjects_seen: NonZeroUsize,
        findings: Vec<Finding>,
    },
    /// Did not run. Cannot be confused with clean by any consumer, because it
    /// is not a list and has no length.
    Withheld(Withheld),
}

impl Evaluated {
    /// Construct a measurement, refusing one over an empty corpus.
    ///
    /// The only constructor. `Measured` cannot be built by hand with a zero
    /// count, which is the whole point: "I examined nothing and found nothing"
    /// has no spelling in this type.
    pub fn measured(subjects_seen: usize, findings: Vec<Finding>) -> Self {
        match NonZeroUsize::new(subjects_seen) {
            Some(seen) => Evaluated::Measured {
                subjects_seen: seen,
                findings,
            },
            None => Evaluated::Withheld(Withheld::InputsAbsent {
                needed: Requires::PathsOnly,
            }),
        }
    }

    pub fn is_clean(&self) -> bool {
        matches!(self, Evaluated::Measured { findings, .. } if findings.is_empty())
    }
}

/// A machine-applicable repair.
///
/// Re-exported from `shape::core::report`, not redefined. This module first
/// declared its own four-variant `Fix` that was a one-for-one re-spelling of
/// the one shape had already shipped -- `MovePath`/`RenameSymbol`/
/// `RetargetDependency`/`CreatePath` against `Move`/`Rename`/`DependOnInstead`/
/// `Create`. Two vocabularies for one concept is the duplication this harness
/// exists to end, and it was introduced by the harness itself.
pub use crate::shape::facade::Fix;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub rule: &'static str,
    /// Ratchet identity. Must survive edits that do not change the defect --
    /// a key derived from a path is invalidated by the rule's own `MovePath`.
    pub key: String,
    pub subject: String,
    pub detail: String,
    pub fix: Option<Fix>,
}

/// A rule's proof that it can fire, and that it does not fire indiscriminately.
///
/// Both halves are required. A rule with only the positive case cannot be shown
/// to discriminate; a rule with only the negative case has never been seen to
/// work at all.
pub struct Fixture {
    pub must_flag: Corpus,
    pub must_pass: Corpus,
}

pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn requires(&self) -> Requires;
    fn examine(&self, corpus: &Corpus) -> Evaluated;
    /// The seeded defect and its conformant twin. Not optional.
    fn fixture(&self) -> Fixture;
}

/// One registration point.
#[derive(Default)]
pub struct Harness {
    rules: Vec<Box<dyn Rule>>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Run {
    pub per_rule: BTreeMap<&'static str, Evaluated>,
}

impl Run {
    /// Rules that did not run, with why. Never empty-by-accident: a rule absent
    /// from `per_rule` is impossible, because the harness inserts an entry for
    /// every registered rule on every path.
    pub fn withheld(&self) -> Vec<(&'static str, &Withheld)> {
        self.per_rule
            .iter()
            .filter_map(|(id, e)| match e {
                Evaluated::Withheld(w) => Some((*id, w)),
                _ => None,
            })
            .collect()
    }

    pub fn findings(&self) -> Vec<&Finding> {
        self.per_rule
            .values()
            .filter_map(|e| match e {
                Evaluated::Measured { findings, .. } => Some(findings),
                _ => None,
            })
            .flatten()
            .collect()
    }

    /// A run is clean only if every rule was measured and none found anything.
    ///
    /// The predicate every consumer needs and none of them had: previously each
    /// asked "are there findings?", which is true of a run that measured
    /// nothing.
    pub fn is_clean(&self) -> bool {
        !self.per_rule.is_empty() && self.per_rule.values().all(Evaluated::is_clean)
    }
}

impl Harness {
    pub fn register(&mut self, rule: Box<dyn Rule>) -> &mut Self {
        self.rules.push(rule);
        self
    }

    pub fn rule_ids(&self) -> Vec<&'static str> {
        self.rules.iter().map(|r| r.id()).collect()
    }

    /// Run every registered rule, self-testing each against its own fixture
    /// first.
    ///
    /// `declared` decides which rules the spec asked for. A rule the engine
    /// knows and the spec omits is `Undeclared` -- present in the output with a
    /// reason, never absent. Eleven rules were previously invisible here.
    pub fn run(&self, corpus: &Corpus, declared: &dyn Fn(&str) -> bool) -> Run {
        let mut out = Run::default();
        for rule in &self.rules {
            let id = rule.id();
            let verdict = if !declared(id) {
                Evaluated::Withheld(Withheld::Undeclared)
            } else if !corpus.satisfies(rule.requires()) {
                Evaluated::Withheld(Withheld::InputsAbsent {
                    needed: rule.requires(),
                })
            } else {
                match self.prove(rule.as_ref()) {
                    Err(detail) => Evaluated::Withheld(Withheld::FixtureFailed { detail }),
                    Ok(()) => rule.examine(corpus),
                }
            };
            out.per_rule.insert(id, verdict);
        }
        out
    }

    /// A rule must flag its seeded defect and spare its conformant twin.
    fn prove(&self, rule: &dyn Rule) -> Result<(), String> {
        let f = rule.fixture();
        match rule.examine(&f.must_flag) {
            Evaluated::Measured { findings, .. } if !findings.is_empty() => {}
            other => {
                return Err(format!(
                    "seeded defect produced no finding ({other:?}); the rule cannot be shown to fire"
                ));
            }
        }
        match rule.examine(&f.must_pass) {
            Evaluated::Measured { findings, .. } if findings.is_empty() => Ok(()),
            other => Err(format!(
                "conformant fixture was flagged ({other:?}); the rule does not discriminate"
            )),
        }
    }
}
