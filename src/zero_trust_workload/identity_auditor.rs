#[derive(Clone, Debug, Default)]
pub struct IdentityAuditor;

impl IdentityAuditor {
    pub fn new() -> Self {
        Self
    }

    pub fn audit_spiffe_and_mtls(&self, diff_content: &str) -> Vec<String> {
        let mut violations = Vec::new();

        for line in diff_content.lines() {
            if line.starts_with('+') {
                let lower = line.to_lowercase();
                if (lower.contains("http://")
                    || lower.contains("insecure_client")
                    || lower.contains("allow_insecure"))
                    && !lower.contains("127.0.0.1")
                    && !lower.contains("localhost")
                {
                    violations.push(format!(
                        "Insecure unencrypted inter-service transport: {}",
                        line.trim()
                    ));
                }
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_insecure_remote_http() {
        let auditor = IdentityAuditor::new();
        let diff = "+ let client = HttpClient::connect(\"http://billing.internal:8080\");";
        let violations = auditor.audit_spiffe_and_mtls(diff);
        assert!(!violations.is_empty());
    }

    #[test]
    fn test_passes_spiffe_mtls_transport() {
        let auditor = IdentityAuditor::new();
        let diff = "+ let client = SpiffeTlsClient::connect(\"https://billing.internal:8443\");";
        let violations = auditor.audit_spiffe_and_mtls(diff);
        assert!(violations.is_empty());
    }
}
