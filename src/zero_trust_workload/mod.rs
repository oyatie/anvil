pub mod identity_auditor;

use identity_auditor::IdentityAuditor;

/// The result of the cleartext-transport lint.
///
/// The `spiffe_id_verified` and `mtls_enforced` booleans this struct used to
/// carry were both assigned `passed` -- the absence of the substring
/// `http://` was published as a statement that a workload had presented an
/// SVID and that a mesh was enforcing mTLS. Nothing here can observe either, so
/// they are gone rather than restated more carefully; see the module docs of
/// `identity_auditor` for why no source-text predicate can replace them.
#[derive(Clone, Debug)]
pub struct ZeroTrustWorkloadReport {
    pub passed: bool,
    pub cleartext_transport_findings: usize,
    /// One entry per finding, so a caller can report which lines rather than
    /// only how many.
    pub violations: Vec<String>,
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

    pub fn evaluate_cleartext_transport(&self, diff_content: &str) -> ZeroTrustWorkloadReport {
        let violations = self.auditor.audit_cleartext_transport(diff_content);
        let cleartext_transport_findings = violations.len();
        let passed = cleartext_transport_findings == 0;

        let summary = if passed {
            "No added line introduces a cleartext endpoint or an explicit insecure-transport \
             opt-in. This is a CWE-319 text lint over the diff; it is not evidence that any \
             workload presented a SPIFFE SVID or that any mesh enforces mTLS."
                .to_string()
        } else {
            format!(
                "{} added line(s) introduce a cleartext endpoint or an explicit \
                 insecure-transport opt-in.",
                cleartext_transport_findings
            )
        };

        ZeroTrustWorkloadReport {
            passed,
            cleartext_transport_findings,
            violations,
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
        let report = gate.evaluate_cleartext_transport(diff);
        assert!(report.passed);
    }
}
