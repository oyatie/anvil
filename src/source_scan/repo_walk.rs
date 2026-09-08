//! What a walk over a repository must not descend into.
//!
//! Split from `source_scan`'s root module, which the 300-line budget bounds:
//! these two predicates answer one question -- which entries a walk over a
//! repository skips -- and the rest of `source_scan` answers about Rust
//! sources.

use std::path::Path;

/// Whether `dir` is the top of a checkout of its own.
///
/// A walk rooted at a repository must not descend into one: another checkout's
/// sources are not this repository's sources, and a census that counts them is
/// not closed over the tree it names. Anvil keeps agent worktrees under
/// `.claude/worktrees/` and a `devtree` beside them, and every root-walking
/// census counted each real site once per checkout (#218).
///
/// # Why `.exists()` and not `.is_dir()` or `.is_file()`
///
/// Both forms occur and neither may be assumed. `git worktree add` writes
/// `.git` as a FILE holding `gitdir: ...`; `git clone` writes it as a
/// DIRECTORY. Measured in this checkout, all three nested checkouts present
/// carry a 64-to-79-byte file and not one is a directory -- so an `is_dir` rule
/// misses every case #218 was filed for, and an `is_file` rule misses every
/// plain nested clone. `tests/source_scan_test.rs` pins both directions
/// against a real filesystem, because a fixture that only ever writes one form
/// leaves the other free to break.
///
/// # What this deliberately also skips
///
/// A submodule. Its `.git` marks source this repository DOES track, through a
/// gitlink, so skipping it under-reports rather than over-reports -- the worse
/// direction. Measured today: no `.gitmodules`, and `git ls-files` names no
/// path under a `.git` component, so nothing is lost. Revisit this the day a
/// submodule is added.
///
/// An IO error answers `false` and the walk descends, which inflates rather
/// than hides. That is the pre-existing direction and not a new hazard.
#[must_use]
pub fn is_separate_checkout(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Whether a walk over a repository should skip this entry entirely.
///
/// The three repository walkers -- the corpus auditor, the freshness ledger
/// and the archival sweeper -- each carried their own copy of this list, and
/// each copy had the same two defects.
///
/// # It was a STRING prefix, so `.github/` was invisible
///
/// `rel.starts_with(".git")` against a `String` is `str::starts_with`, not
/// `Path::starts_with`. A string prefix does not
/// stop at a path boundary, so `.github/`, `.gitignore` and `.gitattributes`
/// all matched `.git`, and `targets/` matched `target`. These walkers scan the
/// repository UNDER REVIEW, where `.github/` routinely holds
/// `ISSUE_TEMPLATE`, `PULL_REQUEST_TEMPLATE` and `CONTRIBUTING.md` -- markdown
/// the auditor exists to audit and never saw. Latent in anvil's own tree,
/// which has no markdown under `.github/`, which is why nothing caught it.
///
/// Matching is now by path COMPONENT, so `.git` skips `.git` and nothing else.
///
/// # It did not skip nested checkouts
///
/// Same defect as #218, one domain over: a contributor with a worktree or a
/// vendored clone inside their repository had every file in it audited as
/// their own. See [`is_separate_checkout`].
/// Takes the repository root and the ABSOLUTE path, and derives the relative
/// one itself. An earlier draft took the relative path and handed it to
/// [`is_separate_checkout`], which tests the filesystem -- so the checkout probe
/// looked at a path relative to the process's working directory and answered
/// about nothing. Both paths are needed and only one of them can be passed
/// wrong, so neither is passed.
#[must_use]
pub fn repository_walk_skips(repo_dir: &Path, path: &Path) -> bool {
    const SKIPPED: &[&str] = &[".git", "target", "buck-out", "node_modules"];
    let relative = path.strip_prefix(repo_dir).unwrap_or(path);
    relative
        .components()
        .any(|c| SKIPPED.contains(&c.as_os_str().to_string_lossy().as_ref()))
        || (path.is_dir() && is_separate_checkout(path))
}
