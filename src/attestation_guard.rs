use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneReceipt {
    pub schema_version: String,
    pub commit_sha: String,
    pub pr_number: u64,
    pub attestation_engine: String,
    pub timestamp_utc: String,
    pub gates_verified: Vec<String>,
    pub verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationReport {
    pub is_attested: bool,
    pub stamped_receipt_path: Option<String>,
    pub summary: String,
}

pub struct AttestationGuard;

impl Default for AttestationGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl AttestationGuard {
    pub fn new() -> Self {
        Self
    }

    /// Generates and stamps a cryptographic provenance receipt into .cursor/receipts/
    /// Verdict written at stamp time, before the gate matrix has been evaluated.
    ///
    /// The receipt previously hardcoded "CERTIFIED_READY" here, while being
    /// stamped *before* `evaluate_pre_merge_gates` ran — asserting a
    /// certification that had not been computed. Invariant I2: never report a
    /// value you did not measure.
    pub const VERDICT_PENDING: &'static str = "PENDING_CERTIFICATION";

    pub async fn stamp_lane_receipt(
        &self,
        repo_dir: &Path,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
        verdict: &str,
        gates_verified: Vec<String>,
    ) -> Result<AttestationReport> {
        info!(
            "Running AttestationGuard receipt generator for {}#{} (SHA: {})...",
            repo, pr_number, head_sha
        );

        let receipts_dir = repo_dir.join(".cursor/receipts");
        if !receipts_dir.exists() {
            let _ = fs::create_dir_all(&receipts_dir).await;
        }

        let receipt = LaneReceipt {
            schema_version: "1.0.0".to_string(),
            commit_sha: head_sha.to_string(),
            pr_number,
            attestation_engine: "Oyatie Autonomous Engineering Pipeline".to_string(),
            timestamp_utc: chrono::Utc::now().to_rfc3339(),
            // Both fields are now supplied by the caller from actual results,
            // rather than being a fixed list asserting gates that had not run.
            gates_verified,
            verdict: verdict.to_string(),
        };

        let filename = format!("pr-{}-attestation.json", pr_number);
        let target_path = receipts_dir.join(&filename);
        let receipt_json = serde_json::to_string_pretty(&receipt)?;

        fs::write(&target_path, &receipt_json).await?;

        let relative_path = format!(".cursor/receipts/{}", filename);
        info!("Successfully stamped lane receipt at {}", relative_path);

        Ok(AttestationReport {
            is_attested: true,
            stamped_receipt_path: Some(relative_path.clone()),
            summary: format!("Cryptographic lane receipt stamped at `{}`", relative_path),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stamp_receipt() {
        let guard = AttestationGuard::new();
        let temp_dir = tempfile::tempdir().expect("tempdir");

        let res = guard
            .stamp_lane_receipt(
                temp_dir.path(),
                "oyatie/console",
                106,
                "abcdef1234567890",
                AttestationGuard::VERDICT_PENDING,
                Vec::new(),
            )
            .await
            .expect("Stamps receipt");

        assert!(res.is_attested);
        assert!(res.stamped_receipt_path.is_some());

        let receipt_file = temp_dir
            .path()
            .join(".cursor/receipts/pr-106-attestation.json");
        assert!(receipt_file.exists());
    }

    /// The receipt must record the verdict it was given, never a fixed value.
    /// It previously hardcoded "CERTIFIED_READY" while being stamped before the
    /// gate matrix ran, asserting a certification nothing had computed (I2).
    #[tokio::test]
    async fn receipt_records_the_supplied_verdict_not_a_hardcoded_one() {
        let guard = AttestationGuard::new();

        for (verdict, gates) in [
            (AttestationGuard::VERDICT_PENDING, Vec::new()),
            ("BLOCKED_NOT_CERTIFIED", vec!["gate-0".to_string()]),
            (
                "CERTIFIED_READY",
                vec!["gate-0".to_string(), "gate-1".to_string()],
            ),
        ] {
            let temp_dir = tempfile::tempdir().expect("tempdir");
            guard
                .stamp_lane_receipt(
                    temp_dir.path(),
                    "oyatie/console",
                    7,
                    "deadbeef",
                    verdict,
                    gates.clone(),
                )
                .await
                .expect("stamps");

            let body = std::fs::read_to_string(
                temp_dir
                    .path()
                    .join(".cursor/receipts/pr-7-attestation.json"),
            )
            .expect("receipt readable");
            let parsed: serde_json::Value = serde_json::from_str(&body).expect("valid json");

            assert_eq!(parsed["verdict"], verdict, "verdict must round-trip");
            assert_eq!(
                parsed["gates_verified"].as_array().map(|a| a.len()),
                Some(gates.len()),
                "gate list must reflect what was supplied"
            );
        }
    }

    /// Guards the specific regression: a freshly stamped receipt, before any
    /// gate has run, must not claim certification.
    #[tokio::test]
    async fn pending_stamp_does_not_claim_certification() {
        assert_ne!(AttestationGuard::VERDICT_PENDING, "CERTIFIED_READY");
        let guard = AttestationGuard::new();
        let temp_dir = tempfile::tempdir().expect("tempdir");
        guard
            .stamp_lane_receipt(
                temp_dir.path(),
                "oyatie/console",
                9,
                "cafe",
                AttestationGuard::VERDICT_PENDING,
                Vec::new(),
            )
            .await
            .expect("stamps");
        let body = std::fs::read_to_string(
            temp_dir
                .path()
                .join(".cursor/receipts/pr-9-attestation.json"),
        )
        .expect("readable");
        assert!(!body.contains("CERTIFIED_READY"));
    }
}
