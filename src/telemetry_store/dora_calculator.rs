use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoraMetricSnapshot {
    pub repo: String,
    pub timestamp: DateTime<Utc>,
    pub lead_time_for_changes_hours: f64,
    pub deployment_frequency_per_day: f64,
    pub change_failure_rate_percent: f64,
    pub mean_time_to_restore_mins: f64,
    pub total_deployments_30d: usize,
    pub total_incidents_30d: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentEvent {
    pub repo: String,
    pub commit_sha: String,
    pub deployed_at: DateTime<Utc>,
    pub environment: String,
    pub is_successful: bool,
    pub lead_time_mins: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentEvent {
    pub repo: String,
    pub incident_id: String,
    pub started_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

pub struct DoraCalculator;

impl DoraCalculator {
    /// Computes DORA metrics across deployment and incident history
    pub fn compute_dora(
        repo: &str,
        deployments: &[DeploymentEvent],
        incidents: &[IncidentEvent],
        window_days: u32,
    ) -> DoraMetricSnapshot {
        let now = Utc::now();
        let cutoff = now - chrono::Duration::days(window_days as i64);

        let window_deployments: Vec<_> = deployments
            .iter()
            .filter(|d| d.repo == repo && d.deployed_at >= cutoff)
            .collect();

        let window_incidents: Vec<_> = incidents
            .iter()
            .filter(|i| i.repo == repo && i.started_at >= cutoff)
            .collect();

        let total_deps = window_deployments.len();
        let dep_freq = if window_days > 0 {
            total_deps as f64 / window_days as f64
        } else {
            0.0
        };

        let lead_time_hours = if !window_deployments.is_empty() {
            let sum_mins: f64 = window_deployments.iter().map(|d| d.lead_time_mins).sum();
            (sum_mins / window_deployments.len() as f64) / 60.0
        } else {
            1.5 // Baseline 1.5h
        };

        let failed_deps = window_deployments
            .iter()
            .filter(|d| !d.is_successful)
            .count();
        let cfr = if total_deps > 0 {
            (failed_deps as f64 / total_deps as f64) * 100.0
        } else {
            0.0
        };

        let mttr_mins = if !window_incidents.is_empty() {
            let mut total_duration_mins = 0.0;
            let mut resolved_count = 0;
            for inc in &window_incidents {
                if let Some(res) = inc.resolved_at {
                    let dur = (res - inc.started_at).num_minutes() as f64;
                    total_duration_mins += dur.max(0.0);
                    resolved_count += 1;
                }
            }
            if resolved_count > 0 {
                total_duration_mins / resolved_count as f64
            } else {
                15.0
            }
        } else {
            10.0 // Baseline 10m
        };

        DoraMetricSnapshot {
            repo: repo.to_string(),
            timestamp: now,
            lead_time_for_changes_hours: lead_time_hours,
            deployment_frequency_per_day: dep_freq,
            change_failure_rate_percent: cfr,
            mean_time_to_restore_mins: mttr_mins,
            total_deployments_30d: total_deps,
            total_incidents_30d: window_incidents.len(),
        }
    }
}
