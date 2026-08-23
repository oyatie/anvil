use regex::Regex;
use serde::{Deserialize, Serialize};

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
const CONVENTIONAL_HEADER: &str =
    r"^(build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test)(\([^()]+\))?!?: +\S";

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

/// Token prefixes that are a credential wherever they appear. Narrow on purpose:
/// each is a fixed vendor prefix with no other meaning, so a hit is evidence
/// rather than a guess.
const SECRET_MARKERS: &[&str] = &["ghp_", "github_pat_", "AKIA", "AWS_SECRET_ACCESS_KEY="];

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
                     build|chore|ci|docs|feat|fix|perf|refactor|revert|style|test."
                )
            },
        })
    }

    /// Scans the staged diff for a credential written into it.
    pub fn scan_staged_diff(&self, staged_diff: &str) -> ProbeFinding {
        let found: Vec<&str> = SECRET_MARKERS
            .iter()
            .copied()
            .filter(|m| staged_diff.contains(m))
            .collect();

        ProbeFinding {
            check_name: "Sub-Second Secret Scan".to_string(),
            is_valid: found.is_empty(),
            message: if found.is_empty() {
                "No token carrying a known credential prefix in the staged diff.".to_string()
            } else {
                format!(
                    "Credential prefix in the staged diff: {}.",
                    found.join(", ")
                )
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
