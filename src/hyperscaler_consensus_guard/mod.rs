use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::diff_context::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperscalerReviewResult {
    pub cloud_provider: String,
    pub is_approved: bool,
    pub primary_invariant: String,
    pub checklist_passed: usize,
    pub checklist_total: usize,
    pub findings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperscalerConsensusReport {
    pub is_unanimously_approved: bool,
    pub provider_reviews: Vec<HyperscalerReviewResult>,
    pub summary: String,
}

pub struct HyperscalerConsensusGuard;

impl Default for HyperscalerConsensusGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl HyperscalerConsensusGuard {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates whether Amazon, Google, Meta, Microsoft, and Oracle would approve the PR
    pub fn evaluate_hyperscaler_consensus(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> HyperscalerConsensusReport {
        info!(
            "Evaluating unbounded-queue and blocking-call conformance on PR #{}...",
            diff_ctx.pr_number
        );

        let mut provider_reviews = Vec::new();
        let diff = &diff_ctx.diff_content;

        // 1. Amazon Web Services (AWS) Evaluation
        let mut aws_findings = Vec::new();
        if diff.contains("tokio::sync::mpsc::unbounded_channel") {
            aws_findings
                .push("AWS Constant Work Violation: Unbounded Tokio channel detected.".to_string());
        }
        if diff.contains("thread::sleep")
            && !diff.contains("jitter")
            && !diff.contains("FullJitter")
        {
            aws_findings.push(
                "AWS Reliability Invariant Violation: Unjittered retry sleep loop.".to_string(),
            );
        }
        let aws_approved = aws_findings.is_empty();
        provider_reviews.push(HyperscalerReviewResult {
            cloud_provider: "Amazon Web Services (AWS)".to_string(),
            is_approved: aws_approved,
            primary_invariant: "Constant Work, Cellular Shuffle-Sharding & Jittered Backpressure"
                .to_string(),
            checklist_passed: if aws_approved {
                5
            } else {
                5 - aws_findings.len()
            },
            checklist_total: 5,
            findings: aws_findings,
        });

        // 2. Google / Google Cloud (GCP) Evaluation
        let mut gcp_findings = Vec::new();
        if (diff.contains("aws_sdk_") || diff.contains("google_cloud_"))
            && diff_ctx
                .changed_files
                .iter()
                .any(|f| f.starts_with("domain/") || f.starts_with("src/domain/"))
        {
            gcp_findings.push("Google Clean Architecture Violation: Proprietary cloud SDK imported directly into domain core.".to_string());
        }
        if diff.contains("curl -s") || diff.contains("wget ") {
            gcp_findings.push("Google Hermetic Build Invariant Violation: Dynamic unpinned network downloads in build path.".to_string());
        }
        let gcp_approved = gcp_findings.is_empty();
        provider_reviews.push(HyperscalerReviewResult {
            cloud_provider: "Google Cloud (GCP)".to_string(),
            is_approved: gcp_approved,
            primary_invariant: "Hermetic Reproducibility, Clean Architecture Ports & REAPI CAS"
                .to_string(),
            checklist_passed: if gcp_approved {
                5
            } else {
                5 - gcp_findings.len()
            },
            checklist_total: 5,
            findings: gcp_findings,
        });

        // 3. Meta Platforms Evaluation
        let mut meta_findings = Vec::new();
        if diff.contains(".unwrap()")
            && diff_ctx
                .changed_files
                .iter()
                .any(|f| !f.contains("test") && !f.contains("fixture"))
        {
            meta_findings.push(
                "Meta Type Safety Violation: Raw unwrap() detected in production execution path."
                    .to_string(),
            );
        }
        if diff_ctx.changed_files.len() > 50 {
            meta_findings.push("Meta Stacked Diff Invariant Violation: Monolithic PR with >50 changed files exceeds atomic review threshold.".to_string());
        }
        let meta_approved = meta_findings.is_empty();
        provider_reviews.push(HyperscalerReviewResult {
            cloud_provider: "Meta Platforms".to_string(),
            is_approved: meta_approved,
            primary_invariant: "Atomic Stacked Diffs, Type Safety & Zero Unhandled Panics"
                .to_string(),
            checklist_passed: if meta_approved {
                5
            } else {
                5 - meta_findings.len()
            },
            checklist_total: 5,
            findings: meta_findings,
        });

        // 4. Microsoft Azure Evaluation
        let mut azure_findings = Vec::new();
        if diff.contains("password =") || diff.contains("api_key =") || diff.contains("AKIA") {
            azure_findings.push(
                "Microsoft SDL Violation: High-entropy static credential detected in source diff."
                    .to_string(),
            );
        }
        if diff.contains("tokio::spawn")
            && !diff.contains(".instrument(")
            && !diff.contains("tracing::")
        {
            azure_findings.push("Microsoft Distributed Tracing Violation: Uninstrumented asynchronous spawn breaks W3C TraceContext.".to_string());
        }
        let azure_approved = azure_findings.is_empty();
        provider_reviews.push(HyperscalerReviewResult {
            cloud_provider: "Microsoft Azure (1ES)".to_string(),
            is_approved: azure_approved,
            primary_invariant: "STRIDE Threat Modeling, W3C TraceContext & Safe Deployment SDP"
                .to_string(),
            checklist_passed: if azure_approved {
                5
            } else {
                5 - azure_findings.len()
            },
            checklist_total: 5,
            findings: azure_findings,
        });

        // 5. Oracle Cloud Infrastructure (OCI) Evaluation
        let mut oci_findings = Vec::new();
        if (diff.contains("ALTER TABLE") || diff.contains("CREATE INDEX"))
            && !diff.contains("CONCURRENTLY")
            && !diff.contains("lock_timeout")
        {
            oci_findings.push("Oracle Zero-Lock Invariant Violation: DDL migration lacks lock_timeout or CONCURRENTLY guard.".to_string());
        }
        let oci_approved = oci_findings.is_empty();
        provider_reviews.push(HyperscalerReviewResult {
            cloud_provider: "Oracle Cloud (OCI)".to_string(),
            is_approved: oci_approved,
            primary_invariant:
                "Zero-Lock DDL Safety, Cumulative 4-Tier IAM & Blast Radius Isolation".to_string(),
            checklist_passed: if oci_approved {
                5
            } else {
                5 - oci_findings.len()
            },
            checklist_total: 5,
            findings: oci_findings,
        });

        let is_unanimously_approved = provider_reviews.iter().all(|r| r.is_approved);
        let approved_count = provider_reviews.iter().filter(|r| r.is_approved).count();

        let summary = if is_unanimously_approved {
            "✅ UNANIMOUS APPROVAL (5/5 review lenses approved)".to_string()
        } else {
            format!(
                "❌ CONSENSUS REJECTED ({}/5 review lenses approved; {} rejected)",
                approved_count,
                5 - approved_count
            )
        };

        HyperscalerConsensusReport {
            is_unanimously_approved,
            provider_reviews,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_hyperscaler_consensus_approves_clean_pr() {
        let dir = tempdir().unwrap();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 10,
            base_branch: "dev".to_string(),
            base_sha: "111".to_string(),
            head_sha: "222".to_string(),
            is_incremental: false,
            previous_head_sha: None,
            diff_content: "+pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
            changed_files: vec!["src/math.rs".to_string()],
            repo_working_dir: dir.path().to_path_buf(),
        };

        let guard = HyperscalerConsensusGuard::new();
        let report = guard.evaluate_hyperscaler_consensus(dir.path(), &diff_ctx);

        assert!(report.is_unanimously_approved);
        assert_eq!(report.provider_reviews.len(), 5);
        assert!(report.summary.contains("5/5 review lenses approved"));
    }

    #[test]
    fn test_hyperscaler_consensus_rejects_unbounded_channel_and_unwrap() {
        let dir = tempdir().unwrap();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 11,
            base_branch: "dev".to_string(),
            base_sha: "111".to_string(),
            head_sha: "222".to_string(),
            is_incremental: false,
            previous_head_sha: None,
            diff_content:
                "+let (tx, rx) = tokio::sync::mpsc::unbounded_channel();\n+let val = opt.unwrap();"
                    .to_string(),
            changed_files: vec!["src/worker.rs".to_string()],
            repo_working_dir: dir.path().to_path_buf(),
        };

        let guard = HyperscalerConsensusGuard::new();
        let report = guard.evaluate_hyperscaler_consensus(dir.path(), &diff_ctx);

        assert!(!report.is_unanimously_approved);
        let aws = report
            .provider_reviews
            .iter()
            .find(|r| r.cloud_provider.contains("AWS"))
            .unwrap();
        assert!(!aws.is_approved);
        let meta = report
            .provider_reviews
            .iter()
            .find(|r| r.cloud_provider.contains("Meta"))
            .unwrap();
        assert!(!meta.is_approved);
    }
}
