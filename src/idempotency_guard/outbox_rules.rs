use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdempotencyFinding {
    pub file_path: String,
    pub endpoint: String,
    pub violation_type: String,
    pub description: String,
}

pub struct OutboxRulesEngine;

impl Default for OutboxRulesEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl OutboxRulesEngine {
    pub fn new() -> Self {
        Self
    }

    /// Scans PR diff for mutating endpoints and checks for idempotency handling
    /// `added` is what this change introduces; `whole` is the hunk it appears
    /// in, context included. The route must be added, but the key that excuses
    /// it may already be there.
    pub fn scan_mutating_endpoints(
        &self,
        file_path: &str,
        added: &str,
        whole: &str,
    ) -> Vec<IdempotencyFinding> {
        let mut findings = Vec::new();
        let post_route_re =
            Regex::new(r#"(?i)route\s*\(\s*["']([^"']+)["']\s*,\s*(?:post|put|delete)\("#).unwrap();
        let idempotency_header_re = Regex::new(r"(?i)Idempotency-Key|idempotency_key").unwrap();

        for line in added.lines() {
            if let Some(caps) = post_route_re.captures(line) {
                // Capture group 1 is the route, and the regex cannot match
                // without it, so there is no case where a name has to be
                // invented. It used to fall back to the literal `unknown`,
                // published in the field an author reads as the endpoint.
                let Some(endpoint) = caps.get(1).map(|m| m.as_str()) else {
                    continue;
                };

                // If mutating route added but no idempotency header is referenced in the file
                if !idempotency_header_re.is_match(whole) {
                    findings.push(IdempotencyFinding {
                        file_path: file_path.to_string(),
                        endpoint: endpoint.to_string(),
                        violation_type: "MISSING_IDEMPOTENCY_KEY".to_string(),
                        description: format!(
                            "Mutating endpoint `{}` declared without validating or extracting an `Idempotency-Key` header.",
                            endpoint
                        ),
                    });
                }
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_mutating_endpoint_missing_idempotency() {
        let engine = OutboxRulesEngine::new();
        let code = r#"
pub fn register_routes(router: Router) -> Router {
    router.route("/api/v1/transfer", post(transfer_funds))
}
"#;
        let findings = engine.scan_mutating_endpoints("src/routes.rs", code, code);
        assert_eq!(findings.len(), 1);
    }
}
