//! A baseline computed from the merge-base tree, rather than committed as a
//! number.
//!
//! A whole-tree count written into the source is a global variable every lane
//! must edit. Two branches that both lower it write the same line and git
//! merges them cleanly, so the merged tree carries a count that is wrong by
//! one with no conflict to catch it — measured, not assumed.
//!
//! Deriving removes the shared edit. Each branch compares against its own
//! merge-base, so nothing is written down, nothing conflicts, and a fall needs
//! no bookkeeping commit.
//!
//! Composed from the two pieces that already exist: `GitMergeBase` resolves
//! the revision, `GitTreeAtRev` loads it.

use crate::ratchet::adapters::GitMergeBase;
use crate::ratchet::ports::{FrozenReferenceSource, RefError};
use crate::shape::facade::{GitTreeAtRev, TreeSource};
use std::path::Path;

/// What a measurement found at the merge-base, and at the head it was run on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Derived<T> {
    pub at_merge_base: T,
    pub merge_base: String,
}

/// Run `measure` against the tree at the merge-base of `head` and `base_ref`.
///
/// `select` decides which files are read, so a scan over Rust sources does not
/// pay to load the whole tree.
pub async fn at_merge_base<T>(
    repo_dir: &Path,
    base_ref: &str,
    head: &str,
    select: impl Fn(&str) -> bool,
    measure: impl Fn(&dyn TreeSource) -> T,
) -> Result<Derived<T>, RefError> {
    let base = GitMergeBase::resolve(repo_dir, base_ref, head).await?;
    let rev = base.reference_rev().to_string();
    let tree = GitTreeAtRev::load(repo_dir, &rev, select)
        .await
        .map_err(|e| RefError::Unavailable(e.to_string()))?;
    Ok(Derived {
        at_merge_base: measure(&tree),
        merge_base: rev,
    })
}

/// Count matching lines across a repository's Rust sources at the merge-base.
///
/// The common shape of a shrink-only source rule: a scan that counts sites and
/// must not grow. `count` receives one file's path and contents and returns
/// how many sites it holds.
///
/// `Ok(None)` when there is no merge-base to compare against — a source
/// tarball, a shallow clone. Withheld rather than passed: a bound nobody
/// computed is not a bound that held.
pub async fn source_sites_at_merge_base(
    repo_dir: &Path,
    base_ref: &str,
    head: &str,
    count: impl Fn(&str, &str) -> usize,
) -> Option<Derived<usize>> {
    at_merge_base(
        repo_dir,
        base_ref,
        head,
        |p| p.starts_with("src/") && p.ends_with(".rs"),
        |tree| {
            tree.paths()
                .iter()
                .filter(|p| p.starts_with("src/") && p.ends_with(".rs"))
                .filter_map(|p| {
                    let bytes = tree.read(p).ok().flatten()?;
                    let text = std::str::from_utf8(bytes).ok()?;
                    Some(count(p, text))
                })
                .sum()
        },
    )
    .await
    .ok()
}
