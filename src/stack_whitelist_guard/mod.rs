use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackWhitelistViolation {
    pub category: String, // "UNAPPROVED_STACK_TECHNOLOGY", "APEX_ADR_IMMUTABILITY_BREACH"
    pub item: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackWhitelistReport {
    pub is_compliant: bool,
    pub violations: Vec<StackWhitelistViolation>,
    pub summary: String,
}

pub struct StackWhitelistGuard;

impl Default for StackWhitelistGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl StackWhitelistGuard {
    pub fn new() -> Self {
        Self
    }

    /// Banned technologies not authorized in apex decisions (ADR-0700..ADR-0718)
    pub const BANNED_UNAPPROVED_STACK: &'static [(&'static str, &'static str)] = &[
        ("redis::", "Redis (Mandate: in-memory LRU/CAS per ADR-0703)"),
        ("mongodb::", "MongoDB (Mandate: PostgreSQL 16 per ADR-0709)"),
        ("mysql::", "MySQL (Mandate: PostgreSQL 16 per ADR-0709)"),
        ("actix_web", "Actix-Web (Mandate: Axum/Tokio per ADR-0701)"),
        ("rocket::", "Rocket (Mandate: Axum/Tokio per ADR-0701)"),
        (
            "cassandra::",
            "Cassandra (Mandate: PostgreSQL 16 per ADR-0709)",
        ),
    ];

    /// Evaluates PR diffs against the Approved Hyperscaler Stack Manifest and Apex ADR Immutability
    pub fn evaluate_stack_whitelist(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        is_human_author: bool,
    ) -> Result<StackWhitelistReport> {
        info!(
            "Running StackWhitelistGuard (Anti-Hallucination & Authority Lock) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut violations = Vec::new();

        // 1. Apex ADR Immutability Lock: Agents cannot modify Accepted Apex ADRs
        for file in &diff_ctx.changed_files {
            if (file.starts_with("docs/decisions/ADR-070")
                || file.starts_with("docs/decisions/ADR-071"))
                && !is_human_author
            {
                violations.push(StackWhitelistViolation {
                    category: "APEX_ADR_IMMUTABILITY_BREACH".to_string(),
                    item: file.clone(),
                    description: format!(
                        "Autonomous agent attempted to modify Accepted Apex decision record '{}'. Apex ADRs are immutable doctrine and require verified human founder approval.",
                        file
                    ),
                });
            }
        }

        // 2. Unapproved Stack / Dependency Hallucination Scanner
        for line in diff_ctx.diff_content.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                for (banned_kw, rationale) in Self::BANNED_UNAPPROVED_STACK {
                    if line.contains(banned_kw) {
                        violations.push(StackWhitelistViolation {
                            category: "UNAPPROVED_STACK_TECHNOLOGY".to_string(),
                            item: banned_kw.to_string(),
                            description: format!(
                                "Unapproved technology '{}' detected in PR diff: {}. Stack additions require an accepted ADR in docs/decisions/.",
                                banned_kw, rationale
                            ),
                        });
                    }
                }
            }
        }

        // 3. Asymmetric Dependency Ratchet: Agents can remove/retire dependencies, but cannot add new ones without ADR
        if !is_human_author {
            for file in &diff_ctx.changed_files {
                if file.ends_with("Cargo.toml") {
                    let mut in_deps_section = false;
                    for line in diff_ctx.diff_content.lines() {
                        if line.contains("[dependencies]")
                            || line.contains("[workspace.dependencies]")
                        {
                            in_deps_section = true;
                        } else if line.starts_with('[') {
                            in_deps_section = false;
                        }

                        if in_deps_section && line.starts_with('+') && !line.starts_with("+++") {
                            let dep_entry = line.trim_start_matches('+').trim();
                            if !dep_entry.is_empty() && !dep_entry.starts_with('#') {
                                violations.push(StackWhitelistViolation {
                                    category: "UNAUTHORIZED_DEPENDENCY_EXPANSION".to_string(),
                                    item: dep_entry.to_string(),
                                    description: format!(
                                        "Autonomous agent attempted to add new dependency '{}' to '{}'. Agents can remove/retire dependencies, but adding new packages requires human approval and an accepted ADR.",
                                        dep_entry, file
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        let is_compliant = violations.is_empty();
        let summary = if is_compliant {
            "Stack Whitelist & Apex Authority Locks verified: 100% compliant with approved hyperscaler architecture.".to_string()
        } else {
            format!(
                "Stack & Authority violations ({} items): {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{}: {}", v.category, v.item))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(StackWhitelistReport {
            is_compliant,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catches_hallucinated_redis_and_apex_adr_mutation() {
        let guard = StackWhitelistGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 999,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "+ use redis::Client;".to_string(),
            changed_files: vec![
                "docs/decisions/ADR-0701-monorepo-capability-live-apex.md".to_string(),
                "crates/cache/src/lib.rs".to_string(),
            ],
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            is_incremental: false,
            previous_head_sha: None,
        };

        let report = guard
            .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, false)
            .unwrap();
        assert!(!report.is_compliant);
        assert!(report
            .violations
            .iter()
            .any(|v| v.category == "APEX_ADR_IMMUTABILITY_BREACH"));
        assert!(report
            .violations
            .iter()
            .any(|v| v.category == "UNAPPROVED_STACK_TECHNOLOGY"));
    }
}
