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

impl StackWhitelistReport {
    /// This report as the certification corpus reads it. See
    /// `CloudNativeReport::gate_status` for why it lives here.
    pub fn gate_status(&self) -> crate::pre_merge_guard::report::GateStatus {
        if self.is_compliant {
            crate::pre_merge_guard::report::GateStatus::Passed
        } else {
            crate::pre_merge_guard::report::GateStatus::Failed(format!(
                "{} added line(s) introduce a technology the approved list does \
                 not name, or edit an accepted apex ADR in place",
                self.violations.len()
            ))
        }
    }
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
            // Per FILE, not per changed-file-path over the whole diff. The
            // section flag is raised by a `[dependencies]` header and lowered
            // by the next line starting `[`; nothing in a `diff --git` header
            // starts with `[`, so scanning the whole diff left the flag raised
            // across every following file. A change that added one dependency
            // and also touched Rust source reported each added source line as
            // an unauthorised dependency, attributed to the Cargo.toml.
            //
            // Scoping the walk to one file's hunks is what makes the flag mean
            // what its name says.
            for fd in crate::git_manager::diff_context::diffs_by_path(&diff_ctx.diff_content) {
                if !fd.path.ends_with("Cargo.toml") {
                    continue;
                }
                let mut in_deps_section = false;
                // `after_change` rather than `added`: the `[dependencies]`
                // header is usually CONTEXT, not an added line, so a rule
                // reading only additions would never see the section it needs
                // to be inside.
                for line in fd.after_change().lines() {
                    let t = line.trim();
                    if t.starts_with("[dependencies]") || t.starts_with("[workspace.dependencies]")
                    {
                        in_deps_section = true;
                        continue;
                    } else if t.starts_with('[') {
                        in_deps_section = false;
                        continue;
                    }
                    if !in_deps_section {
                        continue;
                    }
                    // Only lines this change ADDS are expansions. A dependency
                    // already present is context.
                    if !fd.added().lines().any(|a| a == line) {
                        continue;
                    }
                    let dep_entry = t;
                    if dep_entry.is_empty() || dep_entry.starts_with('#') {
                        continue;
                    }
                    violations.push(StackWhitelistViolation {
                        category: "UNAUTHORIZED_DEPENDENCY_EXPANSION".to_string(),
                        item: dep_entry.to_string(),
                        description: format!(
                            "Autonomous agent attempted to add new dependency '{}' to '{}'. Agents can remove/retire dependencies, but adding new packages requires human approval and an accepted ADR.",
                            dep_entry, fd.path
                        ),
                    });
                }
            }
        }

        let is_compliant = violations.is_empty();
        let summary = if is_compliant {
            "Stack Whitelist & Apex Authority Locks verified: 100% compliant with the approved architecture.".to_string()
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
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("/tmp"),
                crate::git_manager::Uncloned::TestFixture,
            ),
            is_incremental: false,
            previous_head_sha: None,
        };

        let report = guard
            .evaluate_stack_whitelist(Path::new("/tmp"), &diff_ctx, false)
            .unwrap();
        assert!(!report.is_compliant);
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.category == "APEX_ADR_IMMUTABILITY_BREACH")
        );
        assert!(
            report
                .violations
                .iter()
                .any(|v| v.category == "UNAPPROVED_STACK_TECHNOLOGY")
        );
    }
}
