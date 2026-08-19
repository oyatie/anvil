use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocFrontmatter {
    pub schema: Option<String>,
    pub title: Option<String>,
    pub doc_id: Option<String>,
    pub category: Option<String>, // "adr", "contract", "architecture", "runbook", "planning", "archive"
    pub status: Option<String>,   // "active", "draft", "superseded", "deprecated", "archived"
    pub canonical_authority: Option<bool>,
    pub owner: Option<String>,
    pub last_verified_at: Option<String>,
    pub supersedes: Option<Vec<String>>,
    pub superseded_by: Option<Vec<String>>,
}

pub struct FrontmatterValidator;

impl FrontmatterValidator {
    /// Extracts YAML frontmatter bounded by leading `---` delimiters
    pub fn parse_frontmatter(content: &str) -> Option<DocFrontmatter> {
        let trimmed = content.trim_start();
        if !trimmed.starts_with("---") {
            return None;
        }

        let after_first = &trimmed[3..];
        let end_idx = after_first.find("---")?;
        let yaml_str = &after_first[..end_idx];

        serde_yaml::from_str::<DocFrontmatter>(yaml_str).ok()
    }

    /// Validates frontmatter rules and supersession DAGs
    pub fn validate_doc_frontmatter(
        file_path: &str,
        content: &str,
        repo_dir: &Path,
    ) -> Result<(), String> {
        // Only validate Markdown and YAML files in docs/, contracts/, or flags/
        if !file_path.ends_with(".md")
            && !file_path.ends_with(".yaml")
            && !file_path.ends_with(".yml")
        {
            return Ok(());
        }

        let frontmatter = match Self::parse_frontmatter(content) {
            Some(f) => f,
            None => {
                // If in docs/adr/, frontmatter is strictly mandatory
                if file_path.starts_with("docs/adr/") || file_path.starts_with("docs/decisions/") {
                    return Err(format!(
                        "Mandatory YAML frontmatter missing from decision record '{}'",
                        file_path
                    ));
                }
                return Ok(());
            }
        };

        // Rule 1: Canonical Authority Location Check
        let is_canonical_dir =
            file_path.starts_with("docs/") || file_path.starts_with("contracts/");
        if !is_canonical_dir && frontmatter.canonical_authority == Some(true) {
            return Err(format!(
                "File '{}' outside 'docs/' and 'contracts/' cannot declare 'canonical_authority: true'",
                file_path
            ));
        }

        // Rule 2: Supersession DAG Resolution
        if frontmatter.status.as_deref() == Some("superseded")
            || frontmatter.status.as_deref() == Some("Superseded")
        {
            if let Some(superseded_by) = &frontmatter.superseded_by {
                if superseded_by.is_empty() {
                    return Err(format!(
                        "File '{}' marked as superseded must specify valid forward pointer in 'superseded_by'",
                        file_path
                    ));
                }

                // Verify that at least one target pointer exists in docs/ or archive/
                for target in superseded_by {
                    let target_clean = target.trim_start_matches('[').trim_end_matches(']');
                    let adr_path = repo_dir
                        .join("docs/decisions")
                        .join(format!("{}.md", target_clean));
                    let adr_alt = repo_dir
                        .join("docs/adr")
                        .join(format!("{}.md", target_clean));
                    let direct_path = repo_dir.join(target_clean);

                    if !adr_path.exists()
                        && !adr_alt.exists()
                        && !direct_path.exists()
                        && !target_clean.starts_with("ADR-")
                    {
                        return Err(format!(
                            "Superseded doc '{}' specifies nonexistent target '{}'",
                            file_path, target
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_frontmatter() {
        let content = r#"---
schema: hyperscaler.doc.v1
title: Unified Delivery Fabric
status: active
canonical_authority: true
owner: "@team/core"
---
# Content here
"#;
        let fm = FrontmatterValidator::parse_frontmatter(content).expect("Parsed");
        assert_eq!(fm.status.as_deref(), Some("active"));
        assert_eq!(fm.canonical_authority, Some(true));
        assert_eq!(fm.owner.as_deref(), Some("@team/core"));
    }

    #[test]
    fn test_rejects_unauthorized_authority() {
        let content = r#"---
status: active
canonical_authority: true
---"#;
        let res = FrontmatterValidator::validate_doc_frontmatter(
            "tenancy/policy.md",
            content,
            Path::new("/tmp"),
        );
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .contains("outside 'docs/' and 'contracts/'"));
    }
}
