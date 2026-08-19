use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowCadence {
    PerPushPr, // Target: <= 5 min (300s)
    Nightly,   // 5 - 30 min (runs 0 2 * * *)
    Weekly,    // > 30 min (runs 0 3 * * 0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CadenceRoutingFinding {
    pub workflow_file: String,
    pub job_name: String,
    pub measured_seconds: u64,
    pub recommended_cadence: WorkflowCadence,
    pub rationale: String,
}

pub struct CiCadenceClassifier;

impl CiCadenceClassifier {
    pub const PER_PUSH_MAX_WALLCLOCK_SECONDS: u64 = 300; // 5 minutes

    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic classification of CI jobs against the ~5 min per-push ceiling
    pub fn classify_job_cadence(
        &self,
        job_name: &str,
        estimated_seconds: u64,
        is_pr_trigger: bool,
    ) -> Option<CadenceRoutingFinding> {
        if !is_pr_trigger {
            return None;
        }

        if estimated_seconds > Self::PER_PUSH_MAX_WALLCLOCK_SECONDS {
            let (recommended_cadence, schedule) = if estimated_seconds <= 1800 {
                (WorkflowCadence::Nightly, "Nightly cron (`0 2 * * *`)")
            } else {
                (WorkflowCadence::Weekly, "Weekly cron (`0 3 * * 0`)")
            };

            Some(CadenceRoutingFinding {
                workflow_file: ".github/workflows/ci.yaml".to_string(),
                job_name: job_name.to_string(),
                measured_seconds: estimated_seconds,
                recommended_cadence,
                rationale: format!(
                    "Job `{}` estimated at {}s exceeds the ~5min (300s) interactive PR wallclock ceiling. Defer to {} to keep PR feedback loop sub-5min.",
                    job_name, estimated_seconds, schedule
                ),
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classifies_heavy_soak_test_to_nightly() {
        let classifier = CiCadenceClassifier::new();
        let finding = classifier
            .classify_job_cadence("kani_exhaustive_proofs", 480, true) // 8 min
            .unwrap();

        assert_eq!(finding.recommended_cadence, WorkflowCadence::Nightly);
        assert!(finding.rationale.contains("Nightly cron"));
    }

    #[test]
    fn test_passes_fast_pr_job() {
        let classifier = CiCadenceClassifier::new();
        let finding = classifier.classify_job_cadence("pr_light_unit_tests", 140, true);
        assert!(finding.is_none());
    }
}
