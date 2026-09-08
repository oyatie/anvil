//! Reading Rust source as CODE, with its commentary and literal bodies removed.
//!
//! # Why this is one function and not eight
//!
//! Eight implementations of "strip commentary before scanning" existed in this
//! tree under four different behaviours and one name: a four-line filter that
//! drops whole `//` lines, a per-line truncation with a crude quote guard, a
//! line-wise lexer that tracks string literals, and this -- a whole-source state
//! machine that also handles block comments, escapes, and literals spanning
//! lines.
//!
//! That is worse than duplication. A reader seeing `code_only` assumes the
//! strong one and may be handed the weakest, and the failure is silent: a scan
//! that reads a doc comment as code reports a defect that is not there, and one
//! that misses a construct reports nothing at all. Both happened. The
//! line-wise variant could not see that `both_sides(..)` appeared inside a
//! string spanning several lines, so a gate accused its own ledger's prose of
//! being a call site.
//!
//! # Offsets are preserved
//!
//! Everything removed is replaced by spaces rather than deleted, so a byte
//! offset into the result is the same offset in the original. A scanner can
//! report a line and column from it without a second pass.
//!
//! # Two needs, two names
//!
//! A scan looking for a literal MARKER -- the `"+++ b/"` a diff parser keys on,
//! the quoted evidence a citation points at -- must still see string literals;
//! stripping them removes the very thing it is looking for. A scan looking for
//! a construct must NOT see them, or a test fixture quoting `unwrap()` reads as
//! a call to it.
//!
//! Those are different questions and they get different functions:
//! [`code_only`] removes commentary and literal bodies; [`without_commentary`]
//! removes commentary and keeps them. Both preserve offsets. Calling one
//! `code_only` and leaving the caller to guess which behaviour it has is how
//! nine spellings drifted into four behaviours under one name.
//!
//! # What it does not model
//!
//! Raw strings (`r#"..."#`) and a quote inside a character literal. Both are
//! stated rather than implied, because the point of a mechanism is to not
//! overclaim its coverage: either could hide a hit, and neither can invent one.

mod cfg;
#[doc(hidden)]
pub use cfg::Truth as CfgAvailability;
pub mod paths;
mod test_modules;
use std::path::Path;

/// Shared conservative cfg classifier for repository source-analysis gates.
#[doc(hidden)]
pub fn excludes_when_test_is_false(attributes: &[syn::Attribute]) -> bool {
    cfg::excludes_when_test_is_false(attributes)
}

/// Shared tri-state counterpart used by provenance scanners that must retain
/// bindings from every possibly active non-test configuration.
#[doc(hidden)]
pub fn availability_when_test_is_false(attributes: &[syn::Attribute]) -> CfgAvailability {
    cfg::availability_when_test_is_false(attributes)
}

fn mask_character(out: &mut String, character: char) {
    out.extend(std::iter::repeat_n(' ', character.len_utf8()));
}

pub fn code_only(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    let mut in_line_comment = false;
    let mut block_depth = 0usize;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if c == '\n' {
            in_line_comment = false;
            out.push('\n');
            continue;
        }
        if in_line_comment {
            mask_character(&mut out, c);
            continue;
        }
        if block_depth > 0 {
            if c == '/' && chars.peek() == Some(&'*') {
                chars.next();
                block_depth += 1;
                out.push_str("  ");
            } else if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block_depth -= 1;
                out.push_str("  ");
            } else {
                mask_character(&mut out, c);
            }
            continue;
        }
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
                out.push('"');
                continue;
            }
            mask_character(&mut out, c);
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push('"');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            in_line_comment = true;
            out.push(' ');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            block_depth = 1;
            out.push_str("  ");
            continue;
        }
        out.push(c);
    }
    out
}

/// Rust source with its commentary removed and its string literals INTACT.
///
/// For a scan whose subject is a literal: the `"+++ b/"` a diff parser keys on,
/// the quoted evidence a fidelity citation points at. [`code_only`] would strip
/// exactly what such a scan is looking for -- swapping one for the other took a
/// ratchet's count from nineteen sites to two, silently, because every marker
/// it matches is spelled as a string.
///
/// Offsets are preserved, as in [`code_only`]: removed text becomes spaces.
pub fn without_commentary(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    let mut in_str = false;
    let mut in_line_comment = false;
    let mut block_depth = 0usize;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if c == '\n' {
            in_line_comment = false;
            out.push('\n');
            continue;
        }
        if in_line_comment {
            mask_character(&mut out, c);
            continue;
        }
        if block_depth > 0 {
            if c == '/' && chars.peek() == Some(&'*') {
                chars.next();
                block_depth += 1;
                out.push_str("  ");
            } else if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                block_depth -= 1;
                out.push_str("  ");
            } else {
                mask_character(&mut out, c);
            }
            continue;
        }
        if in_str {
            // The body is KEPT. Escapes are still honoured so a `\"` does not
            // end the literal early and leave the scanner reading code as text.
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            out.push(c);
            continue;
        }
        if c == '"' {
            in_str = true;
            out.push(c);
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            in_line_comment = true;
            out.push(' ');
            continue;
        }
        if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            block_depth = 1;
            out.push_str("  ");
            continue;
        }
        out.push(c);
    }
    out
}

/// Whether this file compiles only under `cfg(test)` because its parent
/// module declares it that way.
///
/// Scanners strip test code by finding the literal `#[cfg(test)]` inside a
/// file. That works for `#[cfg(test)] mod tests { … }` written inline, and
/// fails completely for `#[cfg(test)] mod tests;` with the body in a sibling
/// file -- the standard layout the Rust book describes, and the one a module
/// must use once its tests would push it past the 300-line file budget. The
/// attribute is in the PARENT; the file itself carries no marker, so every
/// such scanner reads unit tests as production code.
///
/// Splitting a large guard turns its test functions
/// became, to the diff-parsing ratchet, five new hand-rolled diff parsers.
/// Twelve scanners in this tree strip `#[cfg(test)]` the same way, so the
/// answer belongs here once rather than in each of them.
pub fn is_cfg_test_module_file(repo_root: &Path, path: &Path) -> Result<bool, String> {
    paths::try_is_test_source(repo_root, path)
}

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
/// The skip was written `rel.starts_with(".git")` against a `String`, which is
/// `str::starts_with` and not `Path::starts_with`. A string prefix does not
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

/// Rust source with its `#[cfg(test)]` modules blanked out, line numbering
/// preserved.
///
/// Lives here beside [`code_only`] because it answers the same question about
/// a different axis: that one removes what the compiler ignores, this removes
/// what only the test build compiles. It was private to `brand_absence` until
/// a second caller needed it, which is the point at which a copy would have
/// been the wrong answer.
///
/// Test text never reaches a pull request. Counting a stamp that lives only in
/// a fixture does two kinds of damage: it inflates the debt ledger, and it lets
/// a real production violation hide beneath a ceiling that test data paid for.
///
/// Lines are replaced rather than removed so every reported line number still
/// points at the right line of the original file.
pub fn without_test_modules(source: &str) -> String {
    try_without_test_modules(source).unwrap_or_else(|reason| panic!("{reason}"))
}

/// Fallible form for production gates. Invalid Rust is absent evidence, not a
/// test-free source file.
pub fn try_without_test_modules(source: &str) -> Result<String, String> {
    test_modules::strip(source)
}
