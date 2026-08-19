use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MainlineFailureFinding {
    pub branch: String,
    pub run_id: u64,
    pub workflow_name: String,
    pub failed_job: String,
    pub root_cause_snippet: String,
    pub suggested_remediation: String,
}

pub struct MainlineLogAnalyzer;

impl MainlineLogAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic analysis of failed CI logs on mainline/trunk branches
    pub fn analyze_failed_job_log(
        &self,
        branch: &str,
        run_id: u64,
        job_name: &str,
        log_content: &str,
    ) -> Option<MainlineFailureFinding> {
        if log_content.contains("linker_wrapper.bat")
            && log_content.contains("The system cannot find the path specified")
        {
            return Some(MainlineFailureFinding {
                branch: branch.to_string(),
                run_id,
                workflow_name: "oya-ci-required".to_string(),
                failed_job: job_name.to_string(),
                root_cause_snippet: "MSVC linker (link.exe) not found on windows-latest runner PATH/LIB.".to_string(),
                suggested_remediation: "Add `uses: ilammy/msvc-dev-cmd@v1` on `windows-latest` matrix step in `.github/workflows/oya-ci-required.yml`.".to_string(),
            });
        }

        if log_content.contains("error[E0432]") || log_content.contains("error[E0560]") {
            return Some(MainlineFailureFinding {
                branch: branch.to_string(),
                run_id,
                workflow_name: "oya-ci-required".to_string(),
                failed_job: job_name.to_string(),
                root_cause_snippet: "Rust compilation error on trunk branch.".to_string(),
                suggested_remediation:
                    "Auto-synthesize code fix via Fixer and submit PR to mainline branch."
                        .to_string(),
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diagnoses_windows_msvc_missing() {
        let analyzer = MainlineLogAnalyzer::new();
        let log = "error: linking with linker_wrapper.bat failed: exit code: 1\n= note: The system cannot find the path specified.";
        let finding = analyzer
            .analyze_failed_job_log("dev", 123456, "cross-platform-smoke (windows-latest)", log)
            .unwrap();

        assert_eq!(finding.failed_job, "cross-platform-smoke (windows-latest)");
        assert!(finding.suggested_remediation.contains("msvc-dev-cmd"));
    }
}
