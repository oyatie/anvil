use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeographicScope {
    Global,
    Jurisdiction(String), // e.g. "KR", "US_FED", "US_CA", "EU", "UK", "SG", "JP"
    CloudRegion(String),  // e.g. "ap-northeast-2", "eu-central-1", "us-east-1"
    InternalCorporatePolicy, // Internal Enterprise Security / Legal Doctrine / ADRs
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegulatoryLevel {
    Statute,           // Primary Legislation (법률 / Act)
    EnforcementDecree, // Executive Orders / Decrees (시행령 / Regulation)
    AgencyGuideline, // Regulatory Agency Guidelines & Standards (고시 / 행정지도 / FSS Guidance / FTC Guideline)
    CourtPrecedent,  // Judicial Precedent / Case Law (판례 / 해석례)
    InternalStandard, // Enterprise Architecture Decision / Internal Security Standard
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalValidity {
    pub enacted_date: String,               // e.g. "2024-03-10"
    pub effective_date: String,             // e.g. "2026-09-11"
    pub grace_period_until: Option<String>, // e.g. "2027-03-11" (Advisory mode during grace period)
    pub sunset_date: Option<String>,        // e.g. "2030-12-31" (When repealed/superseded)
}

impl TemporalValidity {
    pub fn is_currently_enforceable(&self, eval_date: &str) -> (bool, bool) {
        let is_effective = eval_date >= self.effective_date.as_str();
        let is_sunset = self
            .sunset_date
            .as_ref()
            .map(|s| eval_date > s.as_str())
            .unwrap_or(false);
        let in_grace_period = self
            .grace_period_until
            .as_ref()
            .map(|g| eval_date <= g.as_str())
            .unwrap_or(false);

        let active = is_effective && !is_sunset;
        let is_advisory_grace = active && in_grace_period;
        (active, is_advisory_grace)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRegulatoryRule {
    pub rule_id: String,
    pub scope: GeographicScope,
    pub level: RegulatoryLevel,
    pub statute_or_policy_name: String,
    pub citation: String,
    pub temporal: TemporalValidity,
    pub official_reference_url: Option<String>,
    pub title: String,
    pub requirement_spec: String,
    pub trigger_paths: Vec<String>, // e.g. ["src/auth/**", "src/billing/**", "migrations/**"]
    pub trigger_extensions: Vec<String>, // e.g. ["rs", "ts", "tsx", "go", "sql"]
    pub pattern_regex: Option<String>,
    pub required_controls: Vec<String>,
    pub severity: String, // "CRITICAL", "HIGH", "MEDIUM", "ADVISORY"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicRegistrySnapshot {
    pub schema_version: String,
    pub last_synced_timestamp: String,
    pub upstream_source: String,
    pub active_jurisdictions: Vec<String>,
    pub total_rules: usize,
    pub rules: Vec<DynamicRegulatoryRule>,
}
