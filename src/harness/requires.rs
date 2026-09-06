//! The ladder of what a rule may ask for, and therefore where it can run.
//!
//! Its own file because it is a contract two modules read from opposite sides:
//! a rule declares its rung, and [`super::corpus::Corpus`] decides whether it
//! holds enough to satisfy one. What the last three rungs hold is in
//! [`super::evidence`].

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
