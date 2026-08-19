pub mod identity_auditor;

use identity_auditor::IdentityAuditor;

#[derive(Clone, Debug)]
pub struct ZeroTrustWorkloadReport {
    pub passed: bool,
    pub spiffe_id_verified: bool,
    pub mtls_enforced: bool,
    pub unauthenticated_endpoints: usize,
    pub summary: String,
}

#[derive(Clone, Debug)]
pub struct ZeroTrustWorkloadGate {
    auditor: IdentityAuditor,
}

impl Default for ZeroTrustWorkloadGate {
    fn default() -> Self {
        Self::new()
    }
}

impl ZeroTrustWorkloadGate {
    pub fn new() -> Self {
        Self {
            auditor: IdentityAuditor::new(),
        }
    }

    pub fn evaluate_workload_identity(&self, diff_content: &str) -> ZeroTrustWorkloadReport {
        let violations = self.auditor.audit_spiffe_and_mtls(diff_content);
        let unauth_count = violations.len();
        let passed = unauth_count == 0;

        let summary = if passed {
            "All inter-service RPCs strictly enforce SPIFFE/SPIRE workload identities and mTLS encryption.".to_string()
        } else {
            format!(
                "Detected {} microservice routes or clients missing SPIFFE workload identity or mTLS.",
                unauth_count
            )
        };

        ZeroTrustWorkloadReport {
            passed,
            spiffe_id_verified: passed,
            mtls_enforced: passed,
            unauthenticated_endpoints: unauth_count,
            summary,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_trust_nominal() {
        let gate = ZeroTrustWorkloadGate::new();
        let diff = "+ let client = TlsClient::with_spiffe_id(\"spiffe://oyatie.internal/ns/prod/sa/order-service\");";
        let report = gate.evaluate_workload_identity(diff);
        assert!(report.passed);
    }
}
