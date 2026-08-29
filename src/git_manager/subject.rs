//! The root of the tree under review, distinguished from any other directory.

use std::path::{Path, PathBuf};

/// A checkout of the repository a review is about.
///
/// The defect this exists to refuse: a gate that means to scan the subject and
/// scans anvil instead. Both are directories, both are `PathBuf`, and the
/// compiler has no opinion about which one a scanner was handed -- so the
/// mistake was caught by reading gate bodies, one gate at a time, and only
/// after a gate had already published a finding against the wrong tree.
///
/// A scanner that takes `&SubjectRoot` cannot be handed anvil's own manifest
/// directory. Not "is checked and reported"; does not compile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectRoot(PathBuf);

impl SubjectRoot {
    /// The clone step, and nothing else.
    ///
    /// `pub(crate)` and called from one place: `GitManager::ensure_repo_cloned`,
    /// which is the only code in this repository that puts a subject on disk.
    pub(crate) fn cloned(dir: PathBuf) -> Self {
        Self(dir)
    }

    /// A root asserted rather than cloned, naming why.
    ///
    /// The escape hatch is deliberate and deliberately awkward. Some callers
    /// genuinely have no clone -- a fixture, a patch with no tree behind it --
    /// and a type with no way to express that would be routed around instead
    /// of used. Requiring a reason from [`Uncloned`] keeps every such site
    /// greppable by symbol rather than by scanning file bodies for the
    /// spellings of "anvil's own directory".
    pub fn asserted(dir: PathBuf, _why: Uncloned) -> Self {
        Self(dir)
    }

    pub fn display(&self) -> std::path::Display<'_> {
        self.0.display()
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, rel: impl AsRef<Path>) -> PathBuf {
        self.0.join(rel)
    }

    /// Whether anything is here at all. A corpus over a bare patch has a root
    /// that names nothing, and a scanner should withhold rather than report it
    /// clean.
    pub fn is_empty(&self) -> bool {
        self.0.as_os_str().is_empty()
    }
}

/// Using a subject where a path is wanted is always sound; the reverse is the
/// defect, and no impl here permits it.
impl AsRef<Path> for SubjectRoot {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

/// Why a [`SubjectRoot`] was asserted instead of cloned.
///
/// Unused at runtime. Its job is to make the caller state, in code a reviewer
/// reads, which of these it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uncloned {
    /// A fixture standing in for a clone, in a test.
    TestFixture,
    /// A patch with no tree behind it: the corpus holds diff text and paths,
    /// and no file may be read.
    NoTreeBehindThisDiff,
    /// Anvil's own tree, measured on purpose.
    ///
    /// The one case where the subject and the reviewer are the same tree.
    /// It is a whole variant so that it reads differently from the defect it
    /// resembles: a gate that meant to scan the subject and reached for
    /// `CARGO_MANIFEST_DIR` instead cannot spell this by accident.
    SelfMeasurement,
    /// A directory named on the command line. The operator is asserting
    /// which tree to work on, which is the one case where a human, and not
    /// the clone step, is the authority.
    OperatorSupplied,
}
