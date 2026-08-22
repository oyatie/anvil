use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyScanResult {
    NoPatternMatched,
    PatternMatched {
        rule_name: String,
        matched_text: String,
        explanation: String,
    },
}

#[derive(Debug, Clone, Default)]
pub struct PolicyPatternScanner;

impl PolicyPatternScanner {
    pub fn new() -> Self {
        Self
    }

    /// Scans policy text for a fixed set of known-dangerous literal patterns.
    ///
    /// This is a keyword scan, not a decision procedure: it proves nothing and
    /// its coverage is exactly the patterns written below. A match is real
    /// evidence; the absence of a match is not evidence of safety.
    pub fn scan_policy_text(&self, policy_content: &str) -> PolicyScanResult {
        // Pattern 1: Catch wildcards in sensitive Cedar resource authorizations
        if policy_content.contains("permit(")
            && (policy_content.contains("principal == Principal::\"*\"")
                || policy_content.contains("permit(principal,")
                || policy_content.contains("permit(principal ,"))
        {
            return PolicyScanResult::PatternMatched {
                rule_name: "CedarPrincipalWildcard".to_string(),
                matched_text: "principal == *".to_string(),
                explanation: "Policy pattern match: wildcard principal allows unauthenticated cross-tenant access".to_string(),
            };
        }

        // Pattern 2: Catch NetworkPolicy egress allowing unrestricted 0.0.0.0/0 to internal ports
        if policy_content.contains("cidr: 0.0.0.0/0") && policy_content.contains("port: 5432") {
            return PolicyScanResult::PatternMatched {
                rule_name: "PostgresPublicEgress".to_string(),
                matched_text: "egress -> 0.0.0.0/0:5432".to_string(),
                explanation:
                    "Policy pattern match: database port 5432 exposed to public CIDR range"
                        .to_string(),
            };
        }

        PolicyScanResult::NoPatternMatched
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smt_catches_wildcard_principal() {
        let engine = PolicyPatternScanner::new();
        let policy = r#"permit(principal == Principal::"*", action == Action::"Read", resource == Resource::"Secret");"#;
        let res = engine.scan_policy_text(policy);
        match res {
            PolicyScanResult::PatternMatched { rule_name, .. } => {
                assert_eq!(rule_name, "CedarPrincipalWildcard");
            }
            PolicyScanResult::NoPatternMatched => panic!("Expected counterexample"),
        }
    }

    #[test]
    fn test_smt_passes_scoped_policy() {
        let engine = PolicyPatternScanner::new();
        let policy = r#"permit(principal == Principal::"User:123", action == Action::"Read", resource == Resource::"Doc:456");"#;
        assert_eq!(
            engine.scan_policy_text(policy),
            PolicyScanResult::NoPatternMatched
        );
    }
}
