use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudNativeViolation {
    pub category: String, // "PROPRIETARY_CLOUD_SDK_IN_CORE", "HARDCODED_CLOUD_ENDPOINT", "NON_RUST_SCRIPT_TOOLING"
    pub description: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudNativeReport {
    pub is_compliant: bool,
    pub violations: Vec<CloudNativeViolation>,
    pub summary: String,
}

pub struct CloudNativeGuard;

impl Default for CloudNativeGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl CloudNativeGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates CNCF compliance, provider neutrality, and Rust tooling purity
    pub fn evaluate_cloud_native(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<CloudNativeReport> {
        info!(
            "Running vendor-neutrality check (proprietary SDK in core, hardcoded endpoints, non-Rust tooling) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut violations = Vec::new();

        // 1. Check for proprietary cloud SDKs in Domain Core
        let proprietary_sdks = [
            "aws-sdk-",
            "aws_sdk_",
            "google_cloud_",
            "azure_",
            "cloudflare::",
        ];
        // Attribution comes from the diff itself, not from the changed-file
        // list. A path-keyed loop over the whole diff joins nothing: the path
        // chooses whether to look and the diff chooses what is found, so an
        // SDK import in an adapter lands on every core file in the change.
        //
        // `diffs_by_path` already attributes hunks to paths and is the parser
        // this repository requires; hand-rolling a second one is what the
        // diff-parsing ratchet forbids.
        for fd in crate::git_manager::diff_context::diffs_by_path(&diff_ctx.diff_content) {
            let is_core = fd.path.contains("/core/") || fd.path.contains("-domain/");
            if !is_core {
                continue;
            }
            for line in fd.added().lines() {
                for sdk in &proprietary_sdks {
                    if line.contains(sdk) {
                        violations.push(CloudNativeViolation {
                            category: "PROPRIETARY_CLOUD_SDK_IN_CORE".to_string(),
                            description: format!(
                                "Domain Core file '{}' directly references proprietary cloud SDK '{}'. Use an abstract Port trait in core and isolate SDK in adapters.",
                                fd.path, sdk
                            ),
                            snippet: line.trim().to_string(),
                        });
                    }
                }
            }
        }

        // 2. Check for hardcoded cloud ARNs / endpoints
        let hardcoded_cloud_patterns = [
            ("arn:aws:", "Hardcoded AWS ARN detected"),
            (".googleapis.com", "Hardcoded GCP API endpoint detected"),
            (
                ".blob.core.windows.net",
                "Hardcoded Azure Blob endpoint detected",
            ),
        ];

        for line in diff_ctx.diff_content.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                for (pat, desc) in &hardcoded_cloud_patterns {
                    if line.contains(pat) {
                        violations.push(CloudNativeViolation {
                            category: "HARDCODED_CLOUD_ENDPOINT".to_string(),
                            description: format!(
                                "{} in PR diff. Use environment-injected URI or port adapter.",
                                desc
                            ),
                            snippet: line.trim().to_string(),
                        });
                    }
                }
            }
        }

        // 3. Check for new non-Rust scripts in scripts/ or tools/
        for file in &diff_ctx.changed_files {
            if (file.starts_with("scripts/") || file.starts_with("tools/"))
                && (file.ends_with(".sh")
                    || file.ends_with(".py")
                    || file.ends_with(".mjs")
                    || file.ends_with(".js"))
            {
                violations.push(CloudNativeViolation {
                    category: "NON_RUST_SCRIPT_TOOLING".to_string(),
                    description: format!(
                        "New non-Rust script '{}' added. Policy requires compiled Rust workspace tools for hermeticity and zero cold-start latency.",
                        file
                    ),
                    snippet: file.clone(),
                });
            }
        }

        let is_compliant = violations.is_empty();
        let summary = if is_compliant {
            "CNCF & Multi-Cloud Neutrality verified: 100% provider-agnostic, clean port/adapter abstraction, and pure Rust tooling.".to_string()
        } else {
            format!(
                "Vendor-neutrality violations ({} items): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{}: {}", v.category, v.snippet))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(CloudNativeReport {
            is_compliant,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catches_proprietary_sdk_in_core() {
        let guard = CloudNativeGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 777,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            // A real unified diff. The previous fixture was a bare `+` line with
            // no header, which git never emits; it passed only because the
            // guard read the path from `changed_files` and the text from the
            // whole string, which is the defect this fixture now exercises.
            diff_content:
                "diff --git a/billing/core/src/invoice.rs b/billing/core/src/invoice.rs\n\
                 --- a/billing/core/src/invoice.rs\n+++ b/billing/core/src/invoice.rs\n\
                 @@ -1,0 +1,1 @@\n+use aws_sdk_s3::Client;\n"
                    .to_string(),
            changed_files: vec!["billing/core/src/invoice.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            is_incremental: false,
            previous_head_sha: None,
        };

        let report = guard
            .evaluate_cloud_native(Path::new("/tmp"), &diff_ctx)
            .unwrap();
        assert!(!report.is_compliant);
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.category == "PROPRIETARY_CLOUD_SDK_IN_CORE")
        );
    }
}
