use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::pre_merge_guard::report::GateStatus;
use crate::pre_merge_guard::scanner::PreMergeScanner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeFinding {
    pub check_name: String,
    pub is_valid: bool,
    pub message: String,
}

/// The Conventional Commits 1.0.0 header, with commitlint's
/// `@commitlint/config-conventional` type list.
///
/// The specification requires `<type>[(scope)][!]: <description>`: the colon and
/// the space after it are mandatory and the description may not be empty, so
/// `feat` and `feat:` are both invalid. The type list is commitlint's default
/// `type-enum`, which is convention rather than specification — the base spec
/// only gives `feat` and `fix` a defined meaning and permits others — and it is
/// matched case-sensitively, which is commitlint's `type-case: lower-case`.
///
/// One deliberate relaxation: more than one space after the colon is accepted.
/// The specification says one, and rejecting the second would be a red an author
/// cannot learn anything from.
///
/// One addition: `promote`. `type-enum` is configuration precisely because it is
/// per-project, and the base specification permits types beyond `feat` and
/// `fix`. This repository's promotion ladder writes `promote(dev): ...` and
/// `promote(staging): ...`; hardcoding commitlint's default and calling it the
/// grammar made the check red on the convention the project actually follows --
/// which is the same shape of invented vocabulary this module exists to delete.
const CONVENTIONAL_HEADER: &str =
    r"^(build|chore|ci|docs|feat|fix|perf|promote|refactor|revert|style|test)(\([^()]+\))?!?: +\S";

/// Subjects git writes rather than the author, taken from commitlint's own
/// `defaultIgnores`. Judging these is a false red nobody can fix: the text is
/// generated, and rewriting it means rewriting history.
const GENERATED_SUBJECT_PREFIXES: &[&str] = &[
    "Merge branch",
    "Merge pull request",
    "Merge remote-tracking branch",
    "Merge tag",
    "fixup!",
    "squash!",
    "amend!",
    "Revert \"",
];

pub struct FastValidator {
    header_re: Regex,
}

impl Default for FastValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl FastValidator {
    pub fn new() -> Self {
        Self {
            header_re: Regex::new(CONVENTIONAL_HEADER)
                .expect("the conventional-commit header pattern is a compile-time constant"),
        }
    }

    /// Judges one commit subject against the Conventional Commits grammar.
    ///
    /// `None` means the subject is one git generated and there is nothing to
    /// judge — which is not the same as a subject that passed, and the caller
    /// must not treat it as one.
    pub fn check_commit_header(&self, commit_msg: &str) -> Option<ProbeFinding> {
        let subject = commit_msg.lines().next().unwrap_or("").trim_end();
        if GENERATED_SUBJECT_PREFIXES
            .iter()
            .any(|p| subject.starts_with(p))
        {
            return None;
        }

        let is_valid = self.header_re.is_match(subject);
        Some(ProbeFinding {
            check_name: "Conventional Commit Header".to_string(),
            is_valid,
            message: if is_valid {
                format!("`{subject}` is a valid conventional commit header.")
            } else {
                format!(
                    "`{subject}` is not a conventional commit header: Conventional Commits \
                     1.0.0 requires <type>[(scope)][!]: <description>, with the colon, the \
                     space and a non-empty description all present, and a type from \
                     build|chore|ci|docs|feat|fix|perf|promote|refactor|revert|style|test."
                )
            },
        })
    }

    /// Scans the lines the staged diff ADDS for a credential written into them.
    ///
    /// Delegates to [`PreMergeScanner::scan_for_secrets`], which this repository
    /// already had: it reads `+` lines only and matches whole credentials
    /// (`AKIA[0-9A-Z]{16}`) rather than prefixes.
    ///
    /// Both properties were defects here. Reading the whole diff meant a pull
    /// request that DELETES a leaked key was refused for containing one -- the
    /// same inversion mutation testing found in `scan_flag_references`, left
    /// live in this scan. And a bare `"AKIA"` substring made every change that
    /// touched the repository's own AWS-key regex block itself.
    pub fn scan_staged_diff(&self, staged_diff: &str) -> ProbeFinding {
        let verdict = PreMergeScanner::scan_for_secrets(staged_diff);
        ProbeFinding {
            check_name: "Sub-Second Secret Scan".to_string(),
            is_valid: verdict.is_acceptable(),
            message: match verdict {
                GateStatus::Failed(why) => format!("{why} (on a line this change adds)."),
                _ => "No credential on a line the staged diff adds.".to_string(),
            },
        }
    }

    /// Both pre-commit checks over one commit message and its staged diff — the
    /// shape a `commit-msg` hook runs, where the real message is available as
    /// `$1`.
    pub fn validate_pre_commit(&self, commit_msg: &str, staged_diff: &str) -> Vec<ProbeFinding> {
        self.check_commit_header(commit_msg)
            .into_iter()
            .chain(std::iter::once(self.scan_staged_diff(staged_diff)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validates_conventional_commit() {
        let validator = FastValidator::new();
        let findings =
            validator.validate_pre_commit("feat(auth): add cedar pdp check", "+ fn ok() {}");
        assert_eq!(findings.len(), 2);
        assert!(findings.iter().all(|f| f.is_valid));
    }

    #[test]
    fn test_catches_invalid_commit_header() {
        let validator = FastValidator::new();
        let findings = validator.validate_pre_commit("updated some stuff", "+ fn ok() {}");
        assert!(!findings[0].is_valid);
    }

    #[test]
    fn test_a_type_prefix_without_a_colon_is_not_a_conventional_commit() {
        let validator = FastValidator::new();
        // `starts_with("feat")` accepted all three; the grammar accepts none.
        for bad in ["feat", "feature: x", "feat:x"] {
            let f = validator.check_commit_header(bad).expect("judged");
            assert!(!f.is_valid, "`{bad}`");
        }
    }

    #[test]
    fn test_a_generated_subject_is_not_judged() {
        let validator = FastValidator::new();
        assert!(
            validator
                .check_commit_header("Merge branch 'main' into x")
                .is_none()
        );
    }
}
