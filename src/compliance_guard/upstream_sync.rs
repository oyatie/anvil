use anyhow::Result;
use std::path::Path;
use std::sync::RwLock;
use tracing::info;

use super::registry::{
    DynamicRegistrySnapshot, DynamicRegulatoryRule, GeographicScope, RegulatoryLevel,
    TemporalValidity,
};

pub struct UpstreamRegulatorySync {
    snapshot: RwLock<DynamicRegistrySnapshot>,
}

impl UpstreamRegulatorySync {
    pub fn new() -> Self {
        let baseline_rules = Self::build_dynamic_living_baseline();
        let total_rules = baseline_rules.len();
        let snapshot = DynamicRegistrySnapshot {
            schema_version: "2026.4.0".to_string(),
            last_synced_timestamp: "2026-08-19T04:53:00Z".to_string(),
            upstream_source:
                "Live Multi-Jurisdiction & Corporate Policy Registry (it-legal + global feeds)"
                    .to_string(),
            active_jurisdictions: vec![
                "KR".to_string(),
                "US_FED".to_string(),
                "US_CA".to_string(),
                "EU".to_string(),
                "GLOBAL_PCI".to_string(),
                "INTERNAL_OYATIE_DOCTRINE".to_string(),
            ],
            total_rules,
            rules: baseline_rules,
        };

        Self {
            snapshot: RwLock::new(snapshot),
        }
    }

    /// Fetches all active regulatory rules filtered by current evaluation date (temporal validity)
    pub fn get_enforceable_rules(&self, current_date: &str) -> Vec<(DynamicRegulatoryRule, bool)> {
        let snap = self.snapshot.read().unwrap();
        snap.rules
            .iter()
            .filter_map(|r| {
                let (active, is_advisory) = r.temporal.is_currently_enforceable(current_date);
                if active {
                    Some((r.clone(), is_advisory))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Dynamically syncs and hot-reloads upstream regulatory rule feeds from a directory or config file
    pub fn sync_from_directory(&self, dir: &Path) -> Result<usize> {
        if !dir.exists() {
            return Ok(0);
        }

        let mut loaded = 0;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(rule) = serde_json::from_str::<DynamicRegulatoryRule>(&content) {
                            let mut snap = self.snapshot.write().unwrap();
                            snap.rules.retain(|r| r.rule_id != rule.rule_id);
                            snap.rules.push(rule);
                            snap.total_rules = snap.rules.len();
                            loaded += 1;
                        }
                    }
                }
            }
        }

        if loaded > 0 {
            info!(
                "UpstreamRegulatorySync: Hot-reloaded {} regulatory rule(s) from {:?}",
                loaded, dir
            );
        }
        Ok(loaded)
    }

    /// Codified multi-jurisdictional living baseline (Korea, US Federal/State, EU, Global, Internal)
    fn build_dynamic_living_baseline() -> Vec<DynamicRegulatoryRule> {
        vec![
            // -------------------------------------------------------------
            // 1. KOREA: PIPA (개인정보 보호법) - Enforced 2026-09-11
            // -------------------------------------------------------------
            DynamicRegulatoryRule {
                rule_id: "KR_PIPA_RRN_BAN".to_string(),
                scope: GeographicScope::Jurisdiction("KR".to_string()),
                level: RegulatoryLevel::Statute,
                statute_or_policy_name: "개인정보 보호법 (PIPA)".to_string(),
                citation: "개보법 §24의2, 영 §21의2".to_string(),
                temporal: TemporalValidity {
                    enacted_date: "2024-03-10".to_string(),
                    effective_date: "2024-03-10".to_string(),
                    grace_period_until: None,
                    sunset_date: None,
                },
                official_reference_url: Some("https://law.go.kr/법령/개인정보보호법".to_string()),
                title: "Resident Registration Number (RRN) Strict Ban".to_string(),
                requirement_spec: "Strict prohibition on handling Korean Resident Registration Numbers without specific statutory mandate. Use CI/DI tokenization.".to_string(),
                trigger_paths: vec!["src/**".into(), "migrations/**".into()],
                trigger_extensions: vec!["rs".into(), "ts".into(), "tsx".into(), "go".into(), "sql".into()],
                pattern_regex: Some(r"\b\d{6}-[1-4]\d{6}\b".to_string()),
                required_controls: vec!["KMC/NICE CI Tokenization".into(), "AES-256 GCM".into()],
                severity: "CRITICAL".to_string(),
            },
            DynamicRegulatoryRule {
                rule_id: "KR_PIPA_INSECURE_HASH".to_string(),
                scope: GeographicScope::Jurisdiction("KR".to_string()),
                level: RegulatoryLevel::AgencyGuideline,
                statute_or_policy_name: "개인정보의 안전성 확보조치 기준 (개인정보보호위원회 고시)".to_string(),
                citation: "개보법 영 §30①4호 가목, 고시 §7".to_string(),
                temporal: TemporalValidity {
                    enacted_date: "2023-09-22".to_string(),
                    effective_date: "2024-01-01".to_string(),
                    grace_period_until: None,
                    sunset_date: None,
                },
                official_reference_url: Some("https://pipc.go.kr".to_string()),
                title: "One-Way Adaptive Password Hashing Mandatory".to_string(),
                requirement_spec: "Passwords must be stored using salted one-way adaptive algorithms (bcrypt, Argon2, scrypt). Legacy single SHA256 or MD5 is illegal.".to_string(),
                trigger_paths: vec!["src/**".into()],
                trigger_extensions: vec!["rs".into(), "ts".into(), "go".into(), "py".into()],
                pattern_regex: Some(r#"(?i)(?:md5|sha1)::digest|createHash\(['"](?:md5|sha1)['"]\)"#.to_string()),
                required_controls: vec!["Argon2id".into(), "bcrypt cost >= 12".into()],
                severity: "CRITICAL".to_string(),
            },

            // -------------------------------------------------------------
            // 2. KOREA: E-COMMERCE ACT (전자상거래법 & 다크패턴) - Enforced 2026-07-21
            // -------------------------------------------------------------
            DynamicRegulatoryRule {
                rule_id: "KR_ECOM_ANTI_DARK_PATTERN_PRECHECK".to_string(),
                scope: GeographicScope::Jurisdiction("KR".to_string()),
                level: RegulatoryLevel::Statute,
                statute_or_policy_name: "전자상거래 등에서의 소비자보호에 관한 법률 (전상법)".to_string(),
                citation: "전상법 §21의2①2호 (다크패턴 금지)".to_string(),
                temporal: TemporalValidity {
                    enacted_date: "2025-01-21".to_string(),
                    effective_date: "2026-07-21".to_string(),
                    grace_period_until: None,
                    sunset_date: None,
                },
                official_reference_url: Some("https://ftc.go.kr".to_string()),
                title: "Pre-Ticked Consent or Add-on Checkbox (Dark Pattern)".to_string(),
                requirement_spec: "Pre-ticking opt-in checkboxes, additional products, or marketing consents is strictly prohibited.".to_string(),
                trigger_paths: vec!["src/**".into(), "web/**".into()],
                trigger_extensions: vec!["tsx".into(), "jsx".into(), "html".into(), "vue".into()],
                pattern_regex: Some(r#"(?i)(?:checked|defaultChecked)\s*=\s*(?:true|\{true\})[\s\S]*?(?:marketing|terms_optional|addon|subscribe_promo)"#.to_string()),
                required_controls: vec!["Affirmative un-checked default".into()],
                severity: "CRITICAL".to_string(),
            },

            // -------------------------------------------------------------
            // 3. UNITED STATES: HIPAA Security Rule (45 CFR §164.312)
            // -------------------------------------------------------------
            DynamicRegulatoryRule {
                rule_id: "US_HIPAA_UNENCRYPTED_EPHI".to_string(),
                scope: GeographicScope::Jurisdiction("US_FED".to_string()),
                level: RegulatoryLevel::EnforcementDecree,
                statute_or_policy_name: "HIPAA Security Rule (45 CFR Part 160 & Part 164)".to_string(),
                citation: "45 CFR §164.312(a)(2)(iv)".to_string(),
                temporal: TemporalValidity {
                    enacted_date: "2003-04-21".to_string(),
                    effective_date: "2003-04-21".to_string(),
                    grace_period_until: None,
                    sunset_date: None,
                },
                official_reference_url: Some("https://hhs.gov/hipaa".to_string()),
                title: "Unencrypted Electronic Protected Health Information (ePHI)".to_string(),
                requirement_spec: "Implement a mechanism to encrypt and decrypt electronic protected health information whenever deemed appropriate.".to_string(),
                trigger_paths: vec!["src/**".into()],
                trigger_extensions: vec!["rs".into(), "ts".into(), "go".into(), "sql".into()],
                pattern_regex: Some(r#"(?i)(?:patient_icd10|medical_record_number|clinical_diagnosis)\s*:\s*String"#.to_string()),
                required_controls: vec!["Field-level encryption at rest".into(), "TLS 1.3 in transit".into()],
                severity: "HIGH".to_string(),
            },

            // -------------------------------------------------------------
            // 4. GLOBAL: PCI-DSS v4.0.1 (Mandatory Effective 2025-03-31)
            // -------------------------------------------------------------
            DynamicRegulatoryRule {
                rule_id: "GLOBAL_PCI_PLAINTEXT_PAN".to_string(),
                scope: GeographicScope::Global,
                level: RegulatoryLevel::AgencyGuideline,
                statute_or_policy_name: "Payment Card Industry Data Security Standard (PCI-DSS v4.0.1)".to_string(),
                citation: "PCI-DSS v4.0.1 Requirement 3.4 & 3.5".to_string(),
                temporal: TemporalValidity {
                    enacted_date: "2022-03-31".to_string(),
                    effective_date: "2024-03-31".to_string(),
                    grace_period_until: Some("2025-03-31".to_string()),
                    sunset_date: None,
                },
                official_reference_url: Some("https://pcisecuritystandards.org".to_string()),
                title: "Plaintext Primary Account Number (Credit Card PAN) Ban".to_string(),
                requirement_spec: "Primary Account Numbers (PAN) must be rendered unreadable anywhere they are stored using strong cryptography.".to_string(),
                trigger_paths: vec!["src/**".into(), "migrations/**".into()],
                trigger_extensions: vec!["rs".into(), "ts".into(), "go".into(), "json".into(), "sql".into()],
                pattern_regex: Some(r"\b(?:4[0-9]{12}(?:[0-9]{3})?|5[1-5][0-9]{14}|3[47][0-9]{13})\b".to_string()),
                required_controls: vec!["Tokenization".into(), "AES-256 GCM".into()],
                severity: "CRITICAL".to_string(),
            },

            // -------------------------------------------------------------
            // 5. INTERNAL ENTERPRISE POLICY: Oyatie Living Security & Architecture Doctrine
            // -------------------------------------------------------------
            DynamicRegulatoryRule {
                rule_id: "INTERNAL_OYATIE_TENANT_ISOLATION_ADR_014".to_string(),
                scope: GeographicScope::InternalCorporatePolicy,
                level: RegulatoryLevel::InternalStandard,
                statute_or_policy_name: "Oyatie Enterprise Architecture Decision Record (ADR-014)".to_string(),
                citation: "ADR-014 §3.1 (Strict Multi-Tenant Cell Boundary)".to_string(),
                temporal: TemporalValidity {
                    enacted_date: "2025-01-01".to_string(),
                    effective_date: "2025-01-01".to_string(),
                    grace_period_until: None,
                    sunset_date: None,
                },
                official_reference_url: Some("docs/adr/0014-tenant-isolation.md".to_string()),
                title: "Strict Multi-Tenant Cell Boundary Enforcement".to_string(),
                requirement_spec: "Every data access query executed against shared cell database clusters must include tenant_id scoping.".to_string(),
                trigger_paths: vec!["src/db/**".into(), "src/models/**".into()],
                trigger_extensions: vec!["rs".into(), "ts".into(), "go".into()],
                pattern_regex: None,
                required_controls: vec!["CellIsolationGuard query filter".into()],
                severity: "CRITICAL".to_string(),
            },
        ]
    }
}
