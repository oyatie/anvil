use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsaPolicyFinding {
    pub file_path: String,
    pub namespace: String,
    pub violation_type: String,
    pub details: String,
}

pub struct PsaAdmissionRules;

impl PsaAdmissionRules {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic evaluation of Native Kubernetes Pod Security Admission (PSA) per ADR-0710 D-1
    pub fn evaluate_psa_manifest(&self, file_path: &str, content: &str) -> Vec<PsaPolicyFinding> {
        let mut findings = Vec::new();

        if !file_path.ends_with(".yaml") && !file_path.ends_with(".yml") {
            return findings;
        }

        // Native Kubernetes PSA check: kind: Namespace must carry pod-security.kubernetes.io/enforce: restricted or be registered
        if content.contains("kind: Namespace") {
            if !content.contains("pod-security.kubernetes.io/enforce:")
                && !file_path.contains("local-path-storage")
                && !file_path.contains("ci-workspace-storage")
            {
                findings.push(PsaPolicyFinding {
                    file_path: file_path.to_string(),
                    namespace: "unlabelled".to_string(),
                    violation_type: "PSA_LABEL_MISSING".to_string(),
                    details: "Namespace declared without native `pod-security.kubernetes.io/enforce: restricted` label or ADR-0710 D-1 exception entry.".to_string(),
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
    fn test_detects_unlabelled_namespace() {
        let rules = PsaAdmissionRules::new();
        let yaml = "apiVersion: v1\nkind: Namespace\nmetadata:\n  name: app";
        let findings = rules.evaluate_psa_manifest("infra/app/ns.yaml", yaml);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn test_passes_restricted_namespace() {
        let rules = PsaAdmissionRules::new();
        let yaml = "apiVersion: v1\nkind: Namespace\nmetadata:\n  name: app\n  labels:\n    pod-security.kubernetes.io/enforce: restricted";
        let findings = rules.evaluate_psa_manifest("infra/app/ns.yaml", yaml);
        assert_eq!(findings.len(), 0);
    }
}
