use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

pub mod telemetry_sentry;
pub use telemetry_sentry::{LiveGoldenSignals, TelemetrySentry};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentSentryReport {
    /// True only when signals were actually read AND were within budget.
    ///
    /// Never true for an unmeasured run. "No data" and "healthy" are different
    /// answers, and a circuit breaker that returns the second for the first
    /// cannot trip.
    pub is_healthy: bool,
    pub should_revert: bool,
    /// Whether live signals were obtained at all.
    ///
    /// The field exists because `is_healthy: false` is ambiguous on its own --
    /// it is both "measured and breaching" and "never measured", and only one
    /// of those is an incident.
    pub measured: bool,
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

        // Signals must come from somewhere. This fabricated all four --
        // p99_latency_ms: 64.0, error_rate_pct: 0.002, panic_count_last_5m: 0
        // -- and fed them to a threshold function whose limits are 500ms, 0.5%
        // and 0 panics. Every literal sat comfortably inside every budget, so
        // the "100% deterministic evaluation of live production incident
        // health" was a constant answering a constant: the breaker reported
        // healthy on every pull request and could not trip on any of them.
        //
        // Anvil has no telemetry endpoint. That is an absence, not a clean bill
        // of health, and this repository already has the vocabulary for the
        // difference. `Absence::NotProvisioned` does not withhold a merge --
        // no author can provision an observability stack in the pull request
        // that trips over its absence -- but it must never be spelled as a
        // pass.
        let signals: Option<LiveGoldenSignals> = live_golden_signals(&diff_ctx.head_sha);

        let Some(signals) = signals else {
            return Ok(IncidentSentryReport {
                is_healthy: false,
                should_revert: false,
                measured: false,
                summary: format!(
                    "NOT MEASURED (no telemetry endpoint is configured, so no \
                     golden signal for {} was read; absence of data is not \
                     evidence of health)",
                    diff_ctx.head_sha
                ),
            });
        };

        let decision = self.sentry.evaluate_production_health(&signals);

        Ok(IncidentSentryReport {
            is_healthy: decision.is_healthy,
            should_revert: decision.should_emergency_revert,
            measured: true,
            summary: decision.notice,
        })
    }
}

/// The live golden signals for a deployed commit, if they can be read.
///
/// `None` today, and honestly so: no telemetry endpoint is configured for any
/// managed repository, so there is nothing to read. This function is the single
/// place that changes when one exists, and its signature is what stops a
/// literal being written at a call site again -- the caller can no longer
/// invent a `LiveGoldenSignals`, it can only ask for one and be told no.
fn live_golden_signals(_deployed_sha: &str) -> Option<LiveGoldenSignals> {
    None
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
        // An unmeasured run raises nothing. `is_healthy` is false both when the
        // deployment is breaching and when no signal was ever read, and only
        // the first is an incident -- keying on it alone would put a standing
        // "the deployment is not healthy" item on the queue for every repo on
        // every sweep, forever, describing a measurement that never happened.
        //
        // The absence itself is real work, but it is a different item with a
        // different remedy (provision telemetry), and it belongs to whoever
        // owns the deployment rather than to this sweep.
        if !self.measured {
            return Vec::new();
        }
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
    fn test_incident_sentry_reports_absence_rather_than_health() {
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
        // This asserted `rep.is_healthy`, which was true because the function
        // built its own signals from four literals that sat inside every
        // threshold. The test certified the constant rather than the system:
        // it would have held with production on fire.
        assert!(!rep.measured, "no telemetry endpoint exists to read");
        assert!(!rep.is_healthy, "an unread signal is not a healthy one");
        assert!(
            !rep.should_revert,
            "and absent data must never trigger the revert either"
        );
    }
}
