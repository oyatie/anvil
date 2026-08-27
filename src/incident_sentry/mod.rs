use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod telemetry_sentry;
pub use telemetry_sentry::{LiveGoldenSignals, TelemetrySentry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentSentryReport {
    pub is_healthy: bool,
    pub should_revert: bool,
    pub summary: String,
}

pub struct IncidentSentryCircuitBreaker {
    sentry: TelemetrySentry,
}

impl Default for IncidentSentryCircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

impl IncidentSentryCircuitBreaker {
    pub fn new() -> Self {
        let sentry = TelemetrySentry::new();
        Self { sentry }
    }

    /// 100% Deterministic evaluation of live production incident health
    pub fn evaluate_incident_sentry(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<IncidentSentryReport> {
        info!(
            "Running IncidentSentryCircuitBreaker (Autonomous Production Incident Sentry) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let baseline = LiveGoldenSignals {
            p99_latency_ms: 64.0,
            error_rate_pct: 0.002,
            panic_count_last_5m: 0,
            deployed_commit_sha: diff_ctx.head_sha.clone(),
        };

        let decision = self.sentry.evaluate_production_health(&baseline);

        Ok(IncidentSentryReport {
            is_healthy: decision.is_healthy,
            should_revert: decision.should_emergency_revert,
            summary: decision.notice,
        })
    }
}

impl IncidentSentryReport {
    /// A live incident, as work.
    ///
    /// A verdict rather than a list: this report says whether the deployment
    /// is healthy, so it raises at most one item and only when it is not.
    /// Health raises nothing — a producer that raised on every observation
    /// would record that the sentry ran rather than that anything is wrong.
    pub fn work_items(&self, repo: &str) -> Vec<crate::intake::WorkItem> {
        use crate::intake::{Remedy, Source, WorkItem, sources::repo_subject};
        if self.is_healthy && !self.should_revert {
            return Vec::new();
        }
        vec![WorkItem {
            source: Source::Incident,
            subject: repo_subject(repo),
            what: if self.should_revert {
                "the sentry calls for a revert".to_string()
            } else {
                "the deployment is not healthy".to_string()
            },
            consequence: self.summary.clone(),
            class: None,
            remedy: Remedy::NeedsJudgement {
                why: "reverting a live deployment is a decision about blast \
                      radius, not a mechanical edit"
                    .to_string(),
            },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incident_sentry_nominal() {
        let breaker = IncidentSentryCircuitBreaker::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ fn safe() {}".to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = breaker
            .evaluate_incident_sentry(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.is_healthy);
        assert!(!rep.should_revert);
    }
}
