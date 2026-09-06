//! The core's own abstraction of a repository tree at one revision: every
//! tracked path, plus the bytes of the files the engine asked to have loaded.
//! Reading a file that was not loaded is an error, never an empty result (I1).
//!
//! Defined in core (the Dependency Rule: core depends on nothing), exposed
//! through `ports` as the seam adapters implement and the facade consumes.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// The source could not be read at all (git failed, path missing).
    Unavailable(String),
    /// The path exists in the tree but its bytes were not loaded.
    NotLoaded(String),
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SourceError::Unavailable(e) => write!(f, "tree source unavailable: {e}"),
            SourceError::NotLoaded(p) => write!(f, "file {p} exists but was not loaded"),
        }
    }
}

impl std::error::Error for SourceError {}

pub trait TreeSource {
    /// The revision this tree was read at (a commit sha, or a label for
    /// in-memory fixtures).
    fn rev(&self) -> &str;
    /// Every tracked file path, repository-relative, forward slashes, sorted.
    fn paths(&self) -> &[String];
    /// Bytes of a loaded file; `Ok(None)` when the path is not in the tree.
    fn read(&self, rel: &str) -> Result<Option<&[u8]>, SourceError>;
    /// Loaded files keyed by path, for callers that iterate manifests.
    fn loaded(&self) -> &BTreeMap<String, Vec<u8>>;

    fn contains(&self, rel: &str) -> bool {
        self.paths()
            .binary_search_by(|p| p.as_str().cmp(rel))
            .is_ok()
    }

    /// Whether any tracked path lies under `dir` (which must end with '/').
    fn has_dir(&self, dir: &str) -> bool {
        let idx = self.paths().partition_point(|p| p.as_str() < dir);
        self.paths().get(idx).is_some_and(|p| p.starts_with(dir))
    }

    /// Every path under `dir` (trailing slash), in order. Paths are sorted,
    /// so the range is one contiguous slice.
    fn under(&self, dir: &str) -> &[String] {
        let paths = self.paths();
        let start = paths.partition_point(|p| p.as_str() < dir);
        let len = paths[start..]
            .iter()
            .take_while(|p| p.starts_with(dir))
            .count();
        &paths[start..start + len]
    }
}
