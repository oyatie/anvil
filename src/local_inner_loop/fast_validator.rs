use serde::{Deserialize, Serialize};

use crate::harness::judgement::ConventionalHeader;
use crate::pre_merge_guard::report::GateStatus;
use crate::pre_merge_guard::scanner::PreMergeScanner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeFinding {
    pub check_name: String,
    pub is_valid: bool,
    pub message: String,
}

pub struct FastValidator {
    header: ConventionalHeader,
}

impl Default for FastValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl FastValidator {
    pub fn new() -> Self {
        Self {
            header: ConventionalHeader::new(),
        }
    }

    /// Judges one commit subject against the Conventional Commits grammar.
    ///
    /// `None` means the subject is one git generated and there is nothing to
    /// judge — which is not the same as a subject that passed, and the caller
    /// must not treat it as one.
    ///
    /// The grammar itself moved to [`crate::harness::judgement`]: this gate is
    /// `Superseded` and the harness rule that shares the judgement is not, so
    /// the dependency runs inward rather than out.
    pub fn check_commit_header(&self, commit_msg: &str) -> Option<ProbeFinding> {
        let verdict = self.header.judge(commit_msg)?;
        Some(ProbeFinding {
            check_name: "Conventional Commit Header".to_string(),
            is_valid: verdict.valid,
            message: verdict.message,
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
