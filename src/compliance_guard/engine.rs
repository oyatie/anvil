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

/// How a line says it is not the thing the rule is looking for.
///
/// Every oracle this gate is measured against has one: Semgrep's `nosemgrep`,
/// Presidio's `allow_list` and `validate_result`, Sensitive Data Protection's
/// `exclusion_rules`. Without one, a repository cannot carry the canonical Visa
/// test PAN in a fixture, which is what made this guard accuse its own author.
///
/// Two things keep it from being a way to switch a statute off. It must name
/// the rule -- there is no blanket form -- so waiving one statute does not
/// waive the rest. And every use is counted into `suppressed_matches` and
/// published in the report's sentence, so a silenced match is visible in the
/// gate's own output rather than free.
pub const SUPPRESSION_MARKER: &str = "anvil-ignore:";

/// Whether `line` waives `rule_id` by name.
fn is_suppressed(line: &str, rule_id: &str) -> bool {
    line.split(SUPPRESSION_MARKER).skip(1).any(|rest| {
        rest.split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|token| token == rule_id)
    })
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

    /// Evaluates PR diffs against active dynamic, temporal, and multi-jurisdictional rules.
    ///
    /// Returns the violations and the number of matches waived by name through
    /// [`SUPPRESSION_MARKER`].
    pub fn scan_diff(
        &self,
        diff_ctx: &PrDiffContext,
        enforceable_rules: &[(DynamicRegulatoryRule, bool)],
    ) -> Result<(Vec<StatutoryViolation>, usize)> {
        let mut violations = Vec::new();
        let mut suppressed = 0usize;
        // Compiled once per rule rather than once per rule per added line. The
        // inner `Regex::new` made a twenty-commit replay of this gate take over
        // two minutes.
        let compiled: Vec<(&DynamicRegulatoryRule, bool, Regex)> = enforceable_rules
            .iter()
            .filter_map(|(rule, grace)| {
                let pattern = rule.pattern_regex.as_ref()?;
                Some((rule, *grace, Regex::new(pattern).ok()?))
            })
            .collect();
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

                for (rule, is_advisory_grace, re) in &compiled {
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

                    if re.is_match(added_code) {
                        if is_suppressed(added_code, &rule.rule_id) {
                            suppressed += 1;
                            continue;
                        }
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

        Ok((violations, suppressed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_waiver_must_name_the_rule_it_waives() {
        assert!(is_suppressed(
            "let x = 1; // anvil-ignore: KR_PIPA_RRN_BAN",
            "KR_PIPA_RRN_BAN"
        ));
        // Naming one statute does not waive another.
        assert!(!is_suppressed(
            "let x = 1; // anvil-ignore: KR_PIPA_RRN_BAN",
            "GLOBAL_PCI_PLAINTEXT_PAN"
        ));
        // There is no blanket form.
        assert!(!is_suppressed(
            "let x = 1; // anvil-ignore",
            "KR_PIPA_RRN_BAN"
        ));
        // A rule id is a whole token, not a prefix.
        assert!(!is_suppressed(
            "let x = 1; // anvil-ignore: TENANT_RULE_2",
            "TENANT_RULE"
        ));
    }
}
