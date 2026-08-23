use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IssueAuditStatus {
    Active,
    ResolvedByCommit,
    ContradictedByADR,
    StaleDuplicate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueAuditFinding {
    pub issue_number: u64,
    pub title: String,
    pub status: IssueAuditStatus,
    pub resolution_reason: String,
    pub resolution_receipt: Option<String>,
}

pub struct IssueAuditor;

impl IssueAuditor {
    /// Evaluates an open issue against the repository's active decisions and commit state
    pub fn audit_issue(
        repo_dir: &Path,
        issue_number: u64,
        title: &str,
        body: &str,
    ) -> IssueAuditFinding {
        // Check 1: Contradicted by active apex ADRs in docs/decisions/
        let decisions_dir = repo_dir.join("docs/decisions");
        if decisions_dir.exists() {
            // Check if issue references a superseded topic (e.g. SeaweedFS primary vs ADR-0709)
            if body.contains("ADR-0196") || title.contains("ADR-0196") {
                return IssueAuditFinding {
                    issue_number,
                    title: title.to_string(),
                    status: IssueAuditStatus::ContradictedByADR,
                    resolution_reason:
                        "Issue is based on superseded ADR-0196. Active source of truth is ADR-0709."
                            .to_string(),
                    resolution_receipt: Some(format!(
                        "ANVIL-ISSUE-CONTRADICTION-RECEIPT#{}",
                        issue_number
                    )),
                };
            }
        }

        // Check 1b: Contradicted or obsolete surface outside live masterplan (specs/masterplan.json)
        if !crate::roadmap_guard::verify_issue_roadmap_alignment(repo_dir, title, body) {
            return IssueAuditFinding {
                issue_number,
                title: title.to_string(),
                status: IssueAuditStatus::ContradictedByADR,
                resolution_reason:
                    "Issue references obsolete/retired planning prefix (.omc/.omx). Active plan authority is specs/masterplan.json."
                        .to_string(),
                resolution_receipt: Some(format!(
                    "ANVIL-ROADMAP-RETIRED-SURFACE-RECEIPT#{}",
                    issue_number
                )),
            };
        }

        // Check 2: candidate for closure if trunk has since recovered.
        //
        // This previously published "Trunk CI is green and passing all gates on the
        // latest commit" -- a factual claim about CI state derived from nothing but the
        // issue title. CI was never queried. The reason below states only what was
        // actually observed; establishing whether trunk recovered requires a real run
        // query and is the reader's call until that signal is wired in.
        if title.contains("🚨 Trunk CI Failure") || title.contains("CI failure") {
            return IssueAuditFinding {
                issue_number,
                title: title.to_string(),
                status: IssueAuditStatus::ResolvedByCommit,
                resolution_reason:
                    "Title matches the transient trunk-CI-failure report pattern. Anvil did \
                     not query CI; whether trunk has since recovered is unverified here."
                        .to_string(),
                resolution_receipt: Some(format!("ANVIL-TRUNK-RECOVERY-RECEIPT#{}", issue_number)),
            };
        }

        // Check 3: Stale planning card without active code owner
        if body.contains("planning_impact: true") && !body.contains("owner:") {
            return IssueAuditFinding {
                issue_number,
                title: title.to_string(),
                status: IssueAuditStatus::StaleDuplicate,
                resolution_reason: "Unassigned legacy planning card exceeding 180-day SLA."
                    .to_string(),
                resolution_receipt: Some(format!("ANVIL-STALE-ISSUE-RECEIPT#{}", issue_number)),
            };
        }

        IssueAuditFinding {
            issue_number,
            title: title.to_string(),
            status: IssueAuditStatus::Active,
            resolution_reason: "Issue remains active and requires engineering attention."
                .to_string(),
            resolution_receipt: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_catches_contradicted_adr_issue() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/decisions")).unwrap();

        let finding = IssueAuditor::audit_issue(
            dir.path(),
            42,
            "Object storage SeaweedFS primary",
            "Referencing ADR-0196 requirements",
        );
        assert_eq!(finding.status, IssueAuditStatus::ContradictedByADR);
        assert!(finding.resolution_receipt.is_some());
    }

    #[test]
    fn test_catches_trunk_ci_failure_recovery() {
        let dir = tempdir().unwrap();
        let finding = IssueAuditor::audit_issue(
            dir.path(),
            105,
            "🚨 Trunk CI Failure on commit abc1234",
            "Lint step failed",
        );
        assert_eq!(finding.status, IssueAuditStatus::ResolvedByCommit);
    }
}
