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
    pub fn scan_mutating_endpoints(
        &self,
        file_path: &str,
        content: &str,
    ) -> Vec<IdempotencyFinding> {
        let mut findings = Vec::new();
        let post_route_re =
            Regex::new(r#"(?i)route\s*\(\s*["']([^"']+)["']\s*,\s*(?:post|put|delete)\("#).unwrap();
        let idempotency_header_re = Regex::new(r"(?i)Idempotency-Key|idempotency_key").unwrap();

        for line in content.lines() {
            if let Some(caps) = post_route_re.captures(line) {
                let endpoint = caps.get(1).map(|m| m.as_str()).unwrap_or("unknown");

                // If mutating route added but no idempotency header is referenced in the file
                if !idempotency_header_re.is_match(content) {
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
        let findings = engine.scan_mutating_endpoints("src/routes.rs", code);
        assert_eq!(findings.len(), 1);
    }
}
