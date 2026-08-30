//! The approved stack, the apex ADR lock, and the dependency ratchet.
//!
//! # Why rule 2 reads `code_only`
//!
//! A bare substring search over every `+` line made this gate refuse the change
//! that WIRED it, twice.
//!
//! First on a test fixture: the change adds `"+use mongodb::Client;\n"` to
//! `tests/stack_whitelist_guard_test.rs` — the line that proves the gate catches
//! MongoDB — and the scan read the fixture as an adoption.
//!
//! Then, after a first fix that stripped only double-quoted spans, on the fix's
//! own comment: `named, not used. Neither can hide a real use redis::…`.
//! Backticks are not quotes and `mod.rs` is not a test source, so the sentence
//! explaining the exclusion was itself read as an adoption of Redis.
//!
//! That was the fourth hand-rolled "is this code" scan in this tree to be beaten
//! by prose, which is why this one is not hand-rolled: `source_scan::code_only`
//! blanks line comments, block comments and string literals in one place that is
//! tested once.
//!
//! Both exclusions are the rule's subject rather than exemptions from it. A gate
//! about production code adopting an unapproved dependency has no subject in a
//! test fixture, and a crate named in a comment or a string is named, not used.
//! Neither hides a real `use redis::…`, which is code in a file that ships.
//!
//! # Why rule 3 walks one file at a time
//!
//! The `[dependencies]` section flag is raised by that header and lowered by
//! the next line starting `[`. Nothing in a `diff --git` header starts with
//! `[`, so scanning the whole diff left the flag raised across every following
//! file: a change that added one dependency and also touched Rust source
//! reported each added source line as an unauthorised dependency, attributed to
//! the Cargo.toml. Scoping the walk to one file's hunks is what makes the flag
//! mean what its name says.

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

    /// Evaluates PR diffs against the approved stack and the apex ADR lock.
    ///
    /// # `is_human_author`
    ///
    /// The certification pipeline does not establish authorship, so neither
    /// value it could pass is a measurement.
    ///
    /// `false` asserts agent authorship nobody observed, and two of the three
    /// rules fire only on that assertion — `APEX_ADR_IMMUTABILITY_BREACH` and
    /// `UNAUTHORIZED_DEPENDENCY_EXPANSION` — so passing it would refuse every
    /// pull request that adds a dependency or touches an ADR, on the strength
    /// of a fact nobody measured. A fabricated accusation is I1's symmetric
    /// violation and the more expensive direction to be wrong in.
    ///
    /// `true` leaves those two rules inert until authorship is measured, which
    /// this gate's fidelity entry records as its gap. The approved-stack rule,
    /// which does not depend on authorship, still runs.
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

        // 2. Unapproved stack scanner. See the module docs.
        let files = crate::git_manager::diff_context::diffs_by_path(&diff_ctx.diff_content);
        if files.is_empty() && !diff_ctx.diff_content.trim().is_empty() {
            // A diff with no `diff --git` headers. Nothing can be attributed to
            // a path, so nothing can be excluded by one either -- and reporting
            // zero violations because the parse found no files is precisely the
            // fail-open this rule exists to prevent. Every added line is
            // scanned, and the finding says it could not name a file.
            for line in diff_ctx.diff_content.lines() {
                if !line.starts_with('+') || line.starts_with("+++") {
                    continue;
                }
                let code = crate::source_scan::code_only(line);
                for (banned_kw, rationale) in Self::BANNED_UNAPPROVED_STACK {
                    if code.contains(banned_kw) {
                        violations.push(StackWhitelistViolation {
                            category: "UNAPPROVED_STACK_TECHNOLOGY".to_string(),
                            item: banned_kw.to_string(),
                            description: format!(
                                "Unapproved technology '{}' added by a diff carrying no file headers, so it cannot be attributed: {}. Stack additions require an accepted ADR in docs/decisions/.",
                                banned_kw, rationale
                            ),
                        });
                    }
                }
            }
        } else {
            for fd in files {
                if crate::source_scan::paths::is_test_source(&fd.path) {
                    continue;
                }
                // `code_only` preserves line count and offsets, so its output
                // lines up with the post-image line for line.
                let after = fd.after_change();
                let code = crate::source_scan::code_only(after);
                let added: Vec<&str> = fd.added().lines().collect();
                for (code_line, raw_line) in code.lines().zip(after.lines()) {
                    if !added.contains(&raw_line) {
                        continue;
                    }
                    for (banned_kw, rationale) in Self::BANNED_UNAPPROVED_STACK {
                        if code_line.contains(banned_kw) {
                            violations.push(StackWhitelistViolation {
                                category: "UNAPPROVED_STACK_TECHNOLOGY".to_string(),
                                item: format!("{}:{}", fd.path, banned_kw),
                                description: format!(
                                    "Unapproved technology '{}' added to '{}': {}. Stack additions require an accepted ADR in docs/decisions/.",
                                    banned_kw, fd.path, rationale
                                ),
                            });
                        }
                    }
                }
            }
        }

        // 3. Asymmetric Dependency Ratchet: Agents can remove/retire dependencies, but cannot add new ones without ADR
        if !is_human_author {
            // Per file. See the module docs for why.
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
mod tests;
