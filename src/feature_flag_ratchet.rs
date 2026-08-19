use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlagViolation {
    pub file_path: String,
    pub flag_name: String,
    pub issue_type: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlagReport {
    pub is_clean: bool,
    pub flags_scanned_count: usize,
    pub violations: Vec<FeatureFlagViolation>,
    pub summary: String,
}

pub struct FeatureFlagRatchet;

impl FeatureFlagRatchet {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates feature flag lifecycle: flags dead toggle branches and encourages flag retirement
    pub fn evaluate_feature_flags(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<FeatureFlagReport> {
        info!(
            "Running FeatureFlagRatchet on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut violations = Vec::new();
        let mut flags_scanned_count = 0;

        let flag_usage_re = Regex::new(r#"(?i)(?:is_feature_enabled|feature_flag|useFeatureFlag|flags\.get)\s*\(\s*["']([^"']+)["']"#).unwrap();
        let permanent_true_re = Regex::new(r#"(?i)if\s+(?:true|1)\s*&&.*is_feature_enabled"#).unwrap();
        let stale_annotation_re = Regex::new(r#"(?i)@deprecated_flag|@stale_flag|EXPIRATION:\s*202[0-5]"#).unwrap();

        let mut current_file = String::new();

        for line in diff_ctx.diff_content.lines() {
            if line.starts_with("+++ b/") {
                current_file = line[6..].trim().to_string();
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let code_line = &line[1..].trim();

                if let Some(caps) = flag_usage_re.captures(code_line) {
                    flags_scanned_count += 1;
                    let flag_name = caps.get(1).map(|m| m.as_str()).unwrap_or("unknown");

                    if permanent_true_re.is_match(code_line) {
                        violations.push(FeatureFlagViolation {
                            file_path: current_file.clone(),
                            flag_name: flag_name.to_string(),
                            issue_type: "DEAD_FLAG_BRANCH".to_string(),
                            description: format!("Feature flag `{}` is hardcoded to active; dead fallback branch should be removed.", flag_name),
                            recommendation: "Remove flag check and delete obsolete fallback branch.".to_string(),
                        });
                    }
                }

                if stale_annotation_re.is_match(code_line) {
                    violations.push(FeatureFlagViolation {
                        file_path: current_file.clone(),
                        flag_name: "stale_annotation".to_string(),
                        issue_type: "EXPIRED_FLAG_PRESENT".to_string(),
                        description: "Found reference to an expired or deprecated feature flag.".to_string(),
                        recommendation: "Retire the flag and prune associated code paths.".to_string(),
                    });
                }
            }
        }

        let is_clean = violations.is_empty();
        let summary = if is_clean {
            if flags_scanned_count > 0 {
                format!(
                    "Feature flag lifecycle clean: {} active flag toggle(s) scanned with zero stale branches.",
                    flags_scanned_count
                )
            } else {
                "Feature flag ratchet verified: zero stale or permanent toggle bloat detected.".to_string()
            }
        } else {
            format!(
                "Feature flag debt detected ({} violation(s)): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{}: {}", v.flag_name, v.issue_type))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(FeatureFlagReport {
            is_clean,
            flags_scanned_count,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_flag_usage_passes() {
        let ratchet = FeatureFlagRatchet::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 301,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/features.ts\n+ if (is_feature_enabled('new_billing_v2')) { doNew(); } else { doOld(); }".to_string(),
            changed_files: vec!["src/features.ts".to_string()],
            is_incremental: false,
        };

        let report = ratchet.evaluate_feature_flags(&temp_dir, &diff_ctx).expect("eval");
        assert!(report.is_clean);
        assert_eq!(report.flags_scanned_count, 1);
    }

    #[test]
    fn test_dead_flag_branch_flags() {
        let ratchet = FeatureFlagRatchet::new();
        let temp_dir = std::env::temp_dir();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 302,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: "+++ b/src/features.ts\n+ // @deprecated_flag\n+ const stale = true;".to_string(),
            changed_files: vec!["src/features.ts".to_string()],
            is_incremental: false,
        };

        let report = ratchet.evaluate_feature_flags(&temp_dir, &diff_ctx).expect("eval");
        assert!(!report.is_clean);
        assert_eq!(report.violations[0].issue_type, "EXPIRED_FLAG_PRESENT");
    }
}
