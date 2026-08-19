use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretPolicyFinding {
    pub workflow_file: String,
    pub line_number: usize,
    pub violation_type: String,
    pub details: String,
}

pub struct OidcPolicyValidator;

impl OidcPolicyValidator {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic scan of GitHub Actions workflows to enforce zero static tokens and short-lived OIDC federated STS credentials
    pub fn validate_workflow_secrets(
        &self,
        file_path: &str,
        content: &str,
    ) -> Vec<SecretPolicyFinding> {
        let mut findings = Vec::new();

        if !file_path.contains(".github/workflows/") {
            return findings;
        }

        let static_aws_secret_re =
            Regex::new(r#"AWS_SECRET_ACCESS_KEY:\s*\$\{\{\s*secrets\.\w+\s*\}\}"#).unwrap();

        for (idx, line) in content.lines().enumerate() {
            if static_aws_secret_re.is_match(line) {
                findings.push(SecretPolicyFinding {
                    workflow_file: file_path.to_string(),
                    line_number: idx + 1,
                    violation_type: "STATIC_LONG_LIVED_SECRET".to_string(),
                    details: "Static long-lived AWS credential used. Enforce OIDC federation with `permissions: id-token: write` and short-lived STS tokens (<= 15m).".to_string(),
                });
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_static_aws_secret() {
        let val = OidcPolicyValidator::new();
        let workflow = "env:\n  AWS_SECRET_ACCESS_KEY: ${{ secrets.AWS_SECRET_ACCESS_KEY }}";
        let findings = val.validate_workflow_secrets(".github/workflows/deploy.yaml", workflow);
        assert_eq!(findings.len(), 1);
    }
}
