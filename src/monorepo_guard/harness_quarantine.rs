use super::MonorepoViolation;

pub struct HarnessQuarantine;

impl HarnessQuarantine {
    pub const BANNED_HARNESS_PREFIXES: &'static [&'static str] = &[
        ".grok/",
        ".claude/",
        ".codex/",
        ".antigravity/",
        ".omx/",
        ".gjc/",
        ".omc/",
        ".gemini/",
    ];

    /// Validates that no AI agent scratch harness or metadata files enter git commits
    pub fn check_harness_quarantine(changed_files: &[String]) -> Vec<MonorepoViolation> {
        let mut violations = Vec::new();

        for file in changed_files {
            for prefix in Self::BANNED_HARNESS_PREFIXES {
                if file.starts_with(prefix) || file.contains(&format!("/{}", prefix)) {
                    violations.push(MonorepoViolation {
                        category: "AI_HARNESS_COMMIT_LEAK".to_string(),
                        description: format!(
                            "AI agent scratch harness file '{}' detected in commit. LLM scratch directories must be added to .gitignore and never committed to trunk.",
                            file
                        ),
                        snippet: file.clone(),
                    });
                    break;
                }
            }
        }

        violations
    }

    /// Validates that canonical authority claims are strictly restricted to docs/ and contracts/
    pub fn check_ssot_authority_location(
        file_path: &str,
        file_content: &str,
    ) -> Option<MonorepoViolation> {
        let is_canonical_location =
            file_path.starts_with("docs/") || file_path.starts_with("contracts/");
        let claims_authority = file_content.contains("canonical_authority: true")
            || (file_content.contains("source of truth") && file_content.contains("canonical"));

        if !is_canonical_location && claims_authority {
            Some(MonorepoViolation {
                category: "UNAUTHORIZED_AUTHORITY_CLAIM".to_string(),
                description: format!(
                    "File '{}' declares canonical authority outside approved SSOT directories ('docs/' or 'contracts/'). Non-canonical files must not declare canonical_authority: true.",
                    file_path
                ),
                snippet: file_path.to_string(),
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catches_grok_and_claude_harness() {
        let changed = vec![
            ".grok/programs/PLAN.md".to_string(),
            ".claude/context.json".to_string(),
            "src/lib.rs".to_string(),
        ];

        let violations = HarnessQuarantine::check_harness_quarantine(&changed);
        assert_eq!(violations.len(), 2);
        assert!(violations.iter().any(|v| v.snippet.contains(".grok")));
        assert!(violations.iter().any(|v| v.snippet.contains(".claude")));
    }

    #[test]
    fn test_catches_unauthorized_ssot_claim() {
        let content = "---\ncanonical_authority: true\n---\n";
        let violation =
            HarnessQuarantine::check_ssot_authority_location("tenancy/policy.md", content);
        assert!(violation.is_some());
        assert_eq!(violation.unwrap().category, "UNAUTHORIZED_AUTHORITY_CLAIM");

        let valid =
            HarnessQuarantine::check_ssot_authority_location("docs/adr/ADR-0701.md", content);
        assert!(valid.is_none());
    }
}
