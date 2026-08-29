//! OpenSLO specification review, and the error-budget burn rate that is *not*
//! measured.
//!
//! # What was here
//!
//! This gate declared a Google SRE multiwindow multi-burn-rate matrix and then
//! assigned its own inputs: a nominal one-hour rate literal compared against a
//! critical-limit literal, and a six-hour pair beside it. Neither figure came
//! from anywhere. The comparison could not cross its bound in any execution, on
//! any pull request, ever — the gate was unfailable by arithmetic, and the
//! scorecard published a PASS quoting those literals as though a measurement had
//! been taken (I2).
//!
//! # What is here now
//!
//! The half that is real is kept: OpenSLO specs named in the diff are read off
//! disk and structurally validated, and a spec that declares no objectives or an
//! out-of-range target is a genuine, reproducible defect that FAILS.
//!
//! The half that was fabricated is deleted outright rather than re-valued. With
//! no telemetry endpoint there is no burn rate, so the gate reports
//! `GateStatus::NotMeasured` naming the missing source — not `Passed`, which
//! would make absent evidence a pass, and not `Failed`, which would accuse every
//! pull request in the fleet of an SLO breach nobody can reproduce (I1 in both
//! directions).
//!
//! A spec named in the diff that cannot be read or parsed is `Errored`: a
//! failure to measure, not a measured failure, and never a silent skip.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod openslo_parser;
pub use openslo_parser::parse_openslo_yaml;

/// What must exist before error-budget consumption can be evaluated at all.
///
/// Published verbatim as the `NotMeasured` reason, so the reader of a pull
/// request sees the missing dependency rather than an absence of comment.
const MISSING_TELEMETRY_SOURCE: &str = "no Prometheus or OpenTelemetry endpoint is configured, so error budget \
     consumption over any window was never queried";

/// Why this gate cannot report a pass in this build, or `None` when it can.
///
/// `SloCanaryGuard::evaluate` below produces exactly three statuses — `Failed`,
/// `Errored`, `NotMeasured` — and no `Passed`, because there is no telemetry
/// source for it to query: nothing in the crate reads a Prometheus or
/// OpenTelemetry endpoint, and there is no configuration field that would name
/// one. All three of those statuses are refused by
/// `PreMergeCertificationReport::admission_refusal`, so no report this build can
/// produce admits a pull request to the merge queue.
///
/// Read by `pre_merge_guard::unmeasurable_gates_in_this_build` so the enlist
/// doors can refuse before paying for a corpus run whose outcome the
/// configuration already fixed. It is a statement about this build, not about
/// any pull request; the gate itself still runs on the review path and still
/// publishes its own `NotMeasured` reason on the scorecard.
///
/// Wiring a telemetry source is what adds the missing `Passed` branch, and this
/// returns `None` from the same change that adds it.
pub fn burn_rate_is_unmeasurable() -> Option<&'static str> {
    Some(MISSING_TELEMETRY_SOURCE)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SloCanaryReport {
    pub status: GateStatus,
    /// No defect was found AND no evidence was lost. False when a spec failed
    /// validation, and also when a spec named in the diff could not be read —
    /// absent evidence must not certify.
    pub is_compliant: bool,
    /// Specs actually parsed. Never incremented for a file that was not read.
    pub slos_evaluated: usize,
    /// Measured defects in the specs that WERE read.
    pub violations: Vec<String>,
    pub summary: String,
}

pub struct SloCanaryGuard;

impl Default for SloCanaryGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl SloCanaryGuard {
    pub fn new() -> Self {
        Self
    }

    /// Validates OpenSLO specs touched by this PR. Error-budget burn rate is
    /// reported as unmeasured; see the module docs.
    pub fn evaluate_slo_canary_health(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<SloCanaryReport> {
        info!(
            "Running SloCanaryGuard (OpenSLO spec validation; burn rate unmeasured) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut slos_evaluated = 0;
        let mut violations = Vec::new();
        // Specs the gate was asked about but could not obtain evidence for.
        // Kept separate from `violations`: a file that could not be read is not
        // a defect in the pull request, it is a defect in the measurement.
        let mut unobtained = Vec::new();

        for file in &diff_ctx.changed_files {
            if !(file.ends_with(".openslo.yaml") || file.ends_with(".openslo.yml")) {
                continue;
            }
            let full_path = repo_dir.join(file);
            match std::fs::read_to_string(&full_path) {
                Ok(content) => match parse_openslo_yaml(&content) {
                    Ok(spec) => {
                        slos_evaluated += 1;
                        if spec.spec.objectives.is_empty() {
                            violations
                                .push(format!("OpenSLO spec `{}` declares 0 objectives", file));
                        }
                        for obj in &spec.spec.objectives {
                            if obj.target <= 0.0 || obj.target > 1.0 {
                                violations.push(format!(
                                    "OpenSLO spec `{}` has invalid target: {} (must be 0.0 < target <= 1.0)",
                                    file, obj.target
                                ));
                            }
                        }
                    }
                    Err(e) => {
                        unobtained.push(format!("OpenSLO YAML parse error in `{}`: {}", file, e));
                    }
                },
                Err(e) => {
                    unobtained.push(format!(
                        "OpenSLO spec `{}` is named in the diff but could not be read: {}",
                        file, e
                    ));
                }
            }
        }

        let is_compliant = violations.is_empty() && unobtained.is_empty();

        // Order matters: a measured defect outranks a failure to measure, and
        // both outrank the absence of a telemetry source.
        let (status, summary) = if !violations.is_empty() {
            let summary = format!(
                "❌ FAILED ({} OpenSLO specification defect(s) across {} spec(s) read)",
                violations.len(),
                slos_evaluated
            );
            (GateStatus::Failed(summary.clone()), summary)
        } else if !unobtained.is_empty() {
            let summary = format!(
                "🛑 ERRORED ({} OpenSLO spec(s) named in the diff could not be read or parsed: {})",
                unobtained.len(),
                unobtained.join("; ")
            );
            (GateStatus::Errored(summary.clone()), summary)
        } else {
            let summary = format!(
                "➖ NOT MEASURED ({}; {} OpenSLO spec(s) read and structurally sound)",
                MISSING_TELEMETRY_SOURCE, slos_evaluated
            );
            (
                GateStatus::NotMeasured {
                    gate_id: "slo_status".to_string(),
                    reason: MISSING_TELEMETRY_SOURCE.to_string(),
                },
                summary,
            )
        };

        Ok(SloCanaryReport {
            status,
            is_compliant,
            slos_evaluated,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(files: &[&str], dir: &Path) -> PrDiffContext {
        PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 10,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: String::new(),
            changed_files: files.iter().map(|f| f.to_string()).collect(),
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                dir.to_path_buf(),
                crate::git_manager::Uncloned::TestFixture,
            ),
            is_incremental: false,
            previous_head_sha: None,
        }
    }

    #[test]
    fn a_pr_with_no_slo_spec_measures_nothing_and_says_so() {
        // Replaces `test_slo_canary_guard_nominal`, which asserted
        // `simulated_burn_rate_5m < max_allowed_burn_rate` -- a comparison
        // between two literals in the same function, true by construction.
        let dir = std::path::PathBuf::from(".");
        let rep = SloCanaryGuard::new()
            .evaluate_slo_canary_health(&dir, &ctx(&["src/main.rs"], &dir))
            .expect("gate runs");
        assert!(matches!(rep.status, GateStatus::NotMeasured { .. }));
        assert_eq!(rep.slos_evaluated, 0);
        assert!(rep.violations.is_empty());
    }

    #[test]
    fn a_spec_that_cannot_be_read_is_not_compliant() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let rep = SloCanaryGuard::new()
            .evaluate_slo_canary_health(tmp.path(), &ctx(&["absent.openslo.yaml"], tmp.path()))
            .expect("gate runs");
        assert!(matches!(rep.status, GateStatus::Errored(_)));
        assert!(!rep.is_compliant, "absent evidence must not certify");
    }
}
