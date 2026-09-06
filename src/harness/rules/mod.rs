//! The registered rules, one per file.
//!
//! A directory rather than a file because the plan converts roughly fifteen
//! more gates into rules, and the one-file form crossed the 300-line budget at
//! four. Splitting after the sixth would have ejected whatever was queued
//! behind it.

pub mod cleartext_transport;
pub mod conventional_commit_subject;
pub mod io_in_pure_face;
pub mod package_name_not_canonical;
pub mod secret_on_added_line;

pub use cleartext_transport::CleartextTransport;
pub use conventional_commit_subject::ConventionalCommitSubject;
pub use io_in_pure_face::IoInPureFace;
pub use package_name_not_canonical::PackageNameNotCanonical;
pub use secret_on_added_line::SecretOnAddedLine;

use super::Harness;

/// Lines a change adds, counted once for every rule that judges them.
///
/// Coverage for a rule at [`super::Requires::Changeset`] is added lines rather
/// than changed files: a change of pure deletions has files and no line such a
/// rule can judge, and `Evaluated::measured` must withhold rather than report
/// it clean.
///
/// Shared rather than written per rule. `tests/diff_parsing_ratchet_test.rs`
/// bounds how many places parse a diff by hand: gates that published a path
/// they had not read out of one each parsed it themselves. A second copy of
/// this loop would be one more.
pub(crate) fn added_line_count(diff: &str) -> usize {
    diff.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .count()
}

/// The one registration point.
pub fn registered() -> Harness {
    let mut h = Harness::default();
    h.register(Box::new(IoInPureFace))
        .register(Box::new(PackageNameNotCanonical))
        .register(Box::new(ConventionalCommitSubject))
        .register(Box::new(SecretOnAddedLine))
        .register(Box::new(CleartextTransport));
    h
}
