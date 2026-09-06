use crate::git_manager::diff_context::diffs_by_path;
use serde::{Deserialize, Serialize};

pub mod policy_scanner;
pub use policy_scanner::{PolicyPatternScanner, PolicyScanResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormalVerificationFinding {
    pub rule: String,
    pub matched_text: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormalVerificationReport {
    pub passed: bool,
    pub findings: Vec<FormalVerificationFinding>,
    /// The policy files this change adds lines to.
    ///
    /// Empty means the scan had nothing to examine, which is NOT the same as a
    /// change whose policy is sound -- and `passed` cannot tell them apart,
    /// because a report with no findings is `passed` either way. The scanner's
    /// own documentation has always said so ("the absence of a match is not
    /// evidence of safety"); until this field existed there was nowhere for a
    /// caller to learn it, so every diff that touched no policy at all
    /// published a green formal-verification gate.
    pub policy_files_seen: Vec<String>,
}

/// Whether a path is one this scan can say anything about.
///
/// STATED COVERAGE, not a guess at intent: the two patterns match Cedar policy
/// text and Kubernetes NetworkPolicy YAML, so those are the files named here. A
/// policy living somewhere this predicate does not recognise is a gap in
/// coverage that shows up as `policy_files_seen` being empty -- reported as
/// unmeasured rather than passed.
pub fn is_policy_path(path: &str) -> bool {
    let p = path.to_ascii_lowercase();
    p.ends_with(".cedar")
        || ["policy", "policies", "netpol", "rbac"]
            .iter()
            .any(|m| p.contains(m))
}

#[derive(Debug, Clone, Default)]
pub struct FormalVerificationGuard {
    solver: PolicyPatternScanner,
}

impl FormalVerificationGuard {
    pub fn new() -> Self {
        Self {
            solver: PolicyPatternScanner::new(),
        }
    }

    /// Scans the policy this change ADDS.
    ///
    /// Two corrections over passing the raw diff straight through:
    ///
    /// * only `+` lines are scanned. The whole diff includes removals, so a
    ///   pull request DELETING `permit(principal == Principal::"*", ...)` was
    ///   failed for containing the wildcard it removes -- the same inversion
    ///   found and fixed in the credential scanner, still live here.
    /// * the policy files touched are recorded, so a caller can tell "scanned
    ///   and found nothing" from "there was no policy to scan".
    pub fn evaluate_formal_invariants(&self, diff_content: &str) -> FormalVerificationReport {
        let mut findings = Vec::new();
        let mut policy_files_seen = Vec::new();
        let mut added = String::new();

        // `diffs_by_path`, not a fourteenth hand-rolled parser.
        //
        // This function walked `+++ b/` headers itself, because it was written
        // before the shared parser existed. The ratchet caught it the moment
        // the two met on a merged tree -- 19 sites became 20 -- which is
        // precisely the job it was added to do, and a better outcome than
        // raising the number.
        //
        // A path is recorded only when the change actually ADDS a line to it:
        // a policy file with deletions alone gives this scan nothing.
        for file in diffs_by_path(diff_content) {
            if !is_policy_path(&file.path) || file.added().is_empty() {
                continue;
            }
            policy_files_seen.push(file.path.clone());
            added.push_str(file.added());
        }

        match self.solver.scan_policy_text(&added) {
            PolicyScanResult::PatternMatched {
                rule_name,
                matched_text,
                explanation,
            } => {
                findings.push(FormalVerificationFinding {
                    rule: rule_name,
                    matched_text,
                    message: explanation,
                });
            }
            PolicyScanResult::NoPatternMatched => {}
        }

        FormalVerificationReport {
            passed: findings.is_empty(),
            findings,
            policy_files_seen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formal_verification_nominal() {
        let guard = FormalVerificationGuard::new();
        let report = guard.evaluate_formal_invariants("let x = 42;");
        assert!(report.passed);
        assert!(report.findings.is_empty());
    }
}
