//! What the three capability rungs hold, now that they hold evidence.
//!
//! `BuildGraph`, `Toolchain` and `Network` were each a `bool` on the corpus. A
//! rule at one of those rungs was told it *may* resolve a dependency graph,
//! invoke cargo, or reach the network -- and then had to do that work inside
//! `Rule::examine`, which is synchronous and returns a verdict.
//!
//! Two consequences followed from the shape rather than from any rule.
//!
//! `Harness::prove` calls `examine` on both halves of every fixture before it
//! will trust a rule, so seventy-two rules is a hundred and forty-four
//! evaluations, each one reaching outward. A fixture is supposed to be the
//! cheapest thing in the system.
//!
//! And the answer could not be reported. A rule that ran a subprocess inside
//! `examine` returns findings; it has nowhere to put "the subprocess did not
//! exist", so an absent toolchain reads as a clean run -- the one confusion
//! [`super::Evaluated`] was built to make unspellable, reintroduced one type
//! below it.
//!
//! Evidence is gathered by the caller, which is where the I/O, the timeouts and
//! the failure modes already live, and handed to `examine` as data. A rule
//! cannot reach the network because there is nothing in the corpus that lets
//! it, which is a stronger statement than a rule that chooses not to.

use std::collections::{BTreeMap, BTreeSet};

/// Resolved dependency edges: a unit, and the units it depends on.
///
/// `BTreeSet` rather than `Vec`: the Dependency Rule asks whether an edge
/// exists, and a graph that answers that twice for one edge has recorded the
/// same fact in two places.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BuildGraph {
    pub edges: BTreeMap<String, BTreeSet<String>>,
}

impl BuildGraph {
    pub fn depends_on(&self, unit: &str, other: &str) -> bool {
        self.edges.get(unit).is_some_and(|d| d.contains(other))
    }
}

/// What one invocation of a toolchain produced.
///
/// `exit_ok` and the streams, and deliberately nothing that interprets them:
/// the rule reading this is the thing that knows what a clippy line means, and
/// a shared interpretation here would be a fourth vocabulary for a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRun {
    /// The program as invoked, for a finding that has to name it.
    pub tool: String,
    pub exit_ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// What one remote query returned.
///
/// The body verbatim. Parsing belongs to the rule that knows the schema, and a
/// query that failed is simply absent from the corpus -- there is no variant
/// here that carries an error, because a rule must not be handed one and left
/// to decide whether it counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// The URL or query this answers, so a finding can cite its source.
    pub source: String,
    pub body: String,
}
