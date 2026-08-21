use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastPreflightFinding {
    pub check_name: String,
    pub details: String,
}

pub struct FastChecksProber;

impl Default for FastChecksProber {
    fn default() -> Self {
        Self::new()
    }
}

impl FastChecksProber {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic sub-second static check: fails fast before spinning up heavy runners
    pub fn probe_static_invariants(&self, diff_content: &str) -> Vec<FastPreflightFinding> {
        let mut findings = Vec::new();

        if diff_content.contains("ghp_") || diff_content.contains("AWS_SECRET_ACCESS_KEY=") {
            findings.push(FastPreflightFinding {
                check_name: "Credential Leaked".to_string(),
                details: "Hardcoded secret key detected in diff. Cancelling matrix execution to avoid compute waste.".to_string(),
            });
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fails_fast_on_secret() {
        let prober = FastChecksProber::new();
        let diff = "+ const token = \"ghp_123456789012345678901234567890123456\";";
        let findings = prober.probe_static_invariants(diff);
        assert_eq!(findings.len(), 1);
    }
}
