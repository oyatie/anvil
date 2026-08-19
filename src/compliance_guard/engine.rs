use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};

use super::registry::DynamicRegulatoryRule;
use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatutoryViolation {
    pub rule_id: String,
    pub scope: String,
    pub regulatory_level: String,
    pub statute_or_policy_name: String,
    pub citation: String,
    pub official_url: Option<String>,
    pub title: String,
    pub severity: String, // "CRITICAL", "HIGH", "MEDIUM", "ADVISORY"
    pub is_grace_period_advisory: bool,
    pub description: String,
    pub file_path: String,
    pub line_snippet: String,
    pub legal_remediation: String,
}

pub struct RegulatoryEngine;

impl Default for RegulatoryEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RegulatoryEngine {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates PR diffs against active dynamic, temporal, and multi-jurisdictional rules
    pub fn scan_diff(
        &self,
        diff_ctx: &PrDiffContext,
        enforceable_rules: &[(DynamicRegulatoryRule, bool)],
    ) -> Result<Vec<StatutoryViolation>> {
        let mut violations = Vec::new();
        let mut current_file = diff_ctx.changed_files.first().cloned().unwrap_or_default();
        let mut current_ext = current_file.rsplit('.').next().unwrap_or("").to_lowercase();

        for line in diff_ctx.diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                current_file = stripped.trim().to_string();
                current_ext = current_file.rsplit('.').next().unwrap_or("").to_lowercase();
                continue;
            }

            if line.starts_with('+') && !line.starts_with("+++") {
                let added_code = &line[1..].trim();

                for (rule, is_advisory_grace) in enforceable_rules {
                    // Check file extension trigger if extension is known
                    if !current_ext.is_empty()
                        && !rule.trigger_extensions.is_empty()
                        && !rule
                            .trigger_extensions
                            .iter()
                            .any(|ext| ext == &current_ext)
                    {
                        continue;
                    }

                    // Regex Pattern check
                    if let Some(ref pattern) = rule.pattern_regex {
                        if let Ok(re) = Regex::new(pattern) {
                            if re.is_match(added_code) {
                                let severity = if *is_advisory_grace {
                                    "ADVISORY".to_string()
                                } else {
                                    rule.severity.clone()
                                };

                                violations.push(StatutoryViolation {
                                    rule_id: rule.rule_id.clone(),
                                    scope: format!("{:?}", rule.scope),
                                    regulatory_level: format!("{:?}", rule.level),
                                    statute_or_policy_name: rule.statute_or_policy_name.clone(),
                                    citation: rule.citation.clone(),
                                    official_url: rule.official_reference_url.clone(),
                                    title: rule.title.clone(),
                                    severity,
                                    is_grace_period_advisory: *is_advisory_grace,
                                    description: rule.requirement_spec.clone(),
                                    file_path: current_file.clone(),
                                    line_snippet: added_code.to_string(),
                                    legal_remediation: format!(
                                        "Align code with statutory mandate: {}. Required controls: {:?}. Citation: {}",
                                        rule.requirement_spec, rule.required_controls, rule.citation
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(violations)
    }
}
