//! Gate 14: are added `unsafe` blocks accompanied by a `// SAFETY:` comment?
//!
//! # What this is, and what it is not
//!
//! This is a comment-presence lint over the diff. It reimplements
//! `clippy::undocumented_unsafe_blocks`, which upstream files under the
//! `restriction` group -- an opt-in documentation policy -- and it establishes
//! exactly one property: a comment exists. It cannot tell whether the comment is
//! true, whether it describes the block below it, or whether the invariants it
//! names hold.
//!
//! No model checker runs here. The module is named `kani_guard` and the gate id
//! is `kani_status` because both are published identifiers that predate this
//! correction; neither Kani, CBMC, Miri, Prusti nor Creusot is invoked at any
//! point, and `proof_runner.rs` -- which once claimed to -- was deleted. The
//! distance between the name and the check is recorded in
//! `src/fidelity/registry.rs` and published on the scorecard.
//!
//! # Known limits of the scan
//!
//! It matches an added line whose first non-space token opens an `unsafe` item,
//! so an `unsafe` block opened mid-line (`let x = unsafe { *p };`) is invisible
//! to it, as is any `unsafe` code this pull request does not touch. Widening the
//! match is not free -- the same widening picks up `unsafe` inside string
//! literals and comments and starts failing pull requests over text -- so the
//! limit is disclosed here and in the fidelity registry rather than papered
//! over by a broader-sounding sentence.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KaniGuardReport {
    /// Every `unsafe` item this scan matched had a `// SAFETY:` comment near it.
    /// A statement about comments, not about memory safety.
    pub all_unsafe_blocks_documented: bool,
    /// Added lines whose first non-space token opens an `unsafe` item.
    pub unsafe_blocks_found: usize,
    /// How many of those had a `// SAFETY:` comment on the line itself or on
    /// one of the five lines before it.
    pub unsafe_blocks_with_safety_comment: usize,
    pub violations: Vec<String>,
    pub summary: String,
}

pub struct KaniGuard;

impl Default for KaniGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl KaniGuard {
    pub fn new() -> Self {
        Self
    }

    /// Reports which `unsafe` items added by this diff carry a `// SAFETY:`
    /// comment. Runs no model checker; see the module docs for the limits.
    pub fn evaluate_unsafe_invariants(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<KaniGuardReport> {
        info!(
            "Running KaniGuard (`// SAFETY:` comment lint over added unsafe blocks) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut unsafe_blocks_found = 0;
        let mut unsafe_blocks_with_safety_comment = 0;
        let mut violations = Vec::new();

        let unsafe_re = Regex::new(r"(?m)^\+\s*unsafe\s*(\{|fn|impl|trait)").unwrap();
        let safety_doc_re = Regex::new(r"(?i)//\s*SAFETY:").unwrap();

        for file_diff in diff_ctx.diff_content.split("diff --git") {
            if !file_diff.contains(".rs") {
                continue;
            }

            let lines: Vec<&str> = file_diff.lines().collect();
            for (idx, line) in lines.iter().enumerate() {
                if unsafe_re.is_match(line) {
                    unsafe_blocks_found += 1;

                    // The five lines above, plus the line itself, are where the
                    // convention puts the comment.
                    let start = idx.saturating_sub(5);
                    let preceding_context = lines[start..idx].join("\n");

                    if safety_doc_re.is_match(&preceding_context) || safety_doc_re.is_match(line) {
                        unsafe_blocks_with_safety_comment += 1;
                    } else {
                        violations.push(format!(
                            "Added `unsafe` block with no `// SAFETY:` comment on it or the 5 lines above it: `{}`",
                            line.trim()
                        ));
                    }
                }
            }
        }

        let all_unsafe_blocks_documented = violations.is_empty();
        let summary = if !all_unsafe_blocks_documented {
            format!(
                "❌ FAILED ({} added `unsafe` block(s) carry no `// SAFETY:` comment)",
                violations.len()
            )
        } else if unsafe_blocks_found == 0 {
            "✅ PASSED (no added line opens an `unsafe` block; this gate reads comments only, no model checker runs)"
                .to_string()
        } else {
            format!(
                "✅ PASSED ({} added `unsafe` block(s), each with a `// SAFETY:` comment; comment presence only, no model checker runs)",
                unsafe_blocks_found
            )
        };

        Ok(KaniGuardReport {
            all_unsafe_blocks_documented,
            unsafe_blocks_found,
            unsafe_blocks_with_safety_comment,
            violations,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kani_guard_passes_safe_diff() {
        let guard = KaniGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content:
                "diff --git a/src/lib.rs b/src/lib.rs\n+pub fn add(a: i32, b: i32) -> i32 { a + b }"
                    .to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard
            .evaluate_unsafe_invariants(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.all_unsafe_blocks_documented);
        assert_eq!(rep.unsafe_blocks_found, 0);
    }

    #[test]
    fn test_kani_guard_flags_undocumented_unsafe() {
        let guard = KaniGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 101,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "diff --git a/src/raw.rs b/src/raw.rs\n+unsafe fn raw_deref(ptr: *const u8) -> u8 { *ptr }".to_string(),
            changed_files: vec!["src/raw.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard
            .evaluate_unsafe_invariants(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(!rep.all_unsafe_blocks_documented);
        assert_eq!(rep.violations.len(), 1);
    }

    #[test]
    fn test_kani_guard_accepts_documented_unsafe() {
        let guard = KaniGuard::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 102,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: "diff --git a/src/raw.rs b/src/raw.rs\n+// SAFETY: the caller upholds non-null and alignment\n+unsafe fn raw_deref(ptr: *const u8) -> u8 { *ptr }".to_string(),
            changed_files: vec!["src/raw.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        };

        let rep = guard
            .evaluate_unsafe_invariants(Path::new("."), &diff_ctx)
            .unwrap();
        assert!(rep.all_unsafe_blocks_documented);
        assert_eq!(rep.unsafe_blocks_found, 1);
        assert_eq!(rep.unsafe_blocks_with_safety_comment, 1);
    }
}
