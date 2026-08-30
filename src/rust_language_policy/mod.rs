use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

pub mod engine;
pub mod scan;

pub use engine::{RustQualityFinding, RustRule};
pub use scan::RustQualityEngine;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::GateStatus;

/// What this run actually looked at.
///
/// There is deliberately no variant that carries a rule count without having
/// run those rules. Both counts the report used to publish were literals: the
/// scanned path claimed `380`, and so did the early return for a diff with no
/// `.rs` file in it — 380 rules reported as evaluated over zero files, under
/// the category "All 27 Categories (Zero Rust files in PR)" and the sentence
/// "rust-skills quality check passed".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RustSkillsMeasurement {
    /// `rust_files` changed `.rs` files were scanned against every rule in
    /// `engine::RULES`.
    Evaluated { rust_files: usize },
    /// The diff changes no `.rs` file. Scope here is the changed-file list
    /// filtered on an extension, which is read exhaustively off the diff the
    /// gate is holding, so this is an observation that there was nothing to
    /// look at — coverage's `NothingToMeasure`, not the empty-scope guess that
    /// `debt_shrink` and `gitops_drift` publish as `NotMeasured`.
    NothingToMeasure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RustSkillsReport {
    pub is_idiomatic: bool,
    pub findings: Vec<RustQualityFinding>,
    /// The size of the ruleset this run evaluated. `engine::RULES.len()` when
    /// something was scanned and `0` when nothing was, never the size of an
    /// upstream corpus nothing here loads.
    pub rules_evaluated_count: usize,
    pub categories_evaluated: Vec<String>,
    pub summary: String,
    /// What was actually scanned, if anything.
    pub measurement: RustSkillsMeasurement,
}

impl RustSkillsReport {
    /// The gate status this report is entitled to publish.
    ///
    /// `NothingToMeasure` is `Passed`, on the rule `trace_context_guard` states
    /// for this repository: a scope read exhaustively off the diff and found
    /// empty is "nothing to look at", which is a pass, while a thing that was
    /// there and could not be judged is `NotMeasured`. The alternative would
    /// deadlock every documentation-only pull request in the fleet — a Rust
    /// idiom gate cannot demand Rust — and it would be an accusation-free
    /// blocker nobody could clear. What the early return may not do is publish
    /// a rule count and the word "passed"; that is fixed in the summary and in
    /// `rules_evaluated_count`, not by inventing a blocker.
    pub fn gate_status(&self) -> GateStatus {
        match self.measurement {
            RustSkillsMeasurement::NothingToMeasure => GateStatus::Passed,
            RustSkillsMeasurement::Evaluated { .. } => {
                if self.is_idiomatic {
                    GateStatus::Passed
                } else {
                    GateStatus::Failed(self.summary.clone())
                }
            }
        }
    }
}

pub struct RustLanguagePolicy {
    engine: RustQualityEngine,
}

impl Default for RustLanguagePolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl RustLanguagePolicy {
    pub fn new() -> Self {
        Self {
            engine: RustQualityEngine::new(),
        }
    }

    /// Scans the `.rs` files this diff changes against the rules
    /// `engine::RULES` lists, which is every rule this process implements.
    ///
    /// The gate is named for `jason931225/rust-skills`, a corpus of 434 rule
    /// files that is not fetched, parsed or consulted anywhere in this binary.
    /// What runs is seven regexes over added diff lines, four of which can
    /// block. The report says so, and says it with numbers taken from the
    /// ruleset that ran.
    pub fn evaluate_rust_quality(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<RustSkillsReport> {
        info!(
            "Running RustLanguagePolicy ({} deterministic Rust rules) on {}#{}...",
            engine::RULES.len(),
            diff_ctx.repo,
            diff_ctx.pr_number
        );

        let rust_files = diff_ctx
            .changed_files
            .iter()
            .filter(|f| f.ends_with(".rs"))
            .count();

        if rust_files == 0 {
            return Ok(RustSkillsReport {
                is_idiomatic: true,
                findings: Vec::new(),
                rules_evaluated_count: 0,
                categories_evaluated: Vec::new(),
                summary: "Rust idiom scan: this diff changes no `.rs` file, so no rule was \
                          evaluated and nothing is claimed about the Rust in this repository."
                    .to_string(),
                measurement: RustSkillsMeasurement::NothingToMeasure,
            });
        }

        let findings = self.engine.scan_diff(diff_ctx)?;

        // Through `RULES` by rule id, not by re-testing the severity string: the
        // count published beside this ("N rule(s), M of which can block") comes
        // from the same table, so the sentence and the behaviour cannot disagree.
        let is_idiomatic = !findings.iter().any(|f| f.blocks());

        let blocking = engine::RULES.iter().filter(|r| r.blocks()).count();
        let summary = if is_idiomatic {
            if findings.is_empty() {
                format!(
                    "Rust idiom scan: {} rule(s), {} of which can block, matched nothing in the \
                     {} changed `.rs` file(s). This is a regex scan over added lines, not a \
                     compilation or a clippy run.",
                    engine::RULES.len(),
                    blocking,
                    rust_files
                )
            } else {
                format!(
                    "Rust idiom scan: {} non-blocking recommendation(s) across {} changed `.rs` \
                     file(s), from {} rule(s).",
                    findings.len(),
                    rust_files,
                    engine::RULES.len()
                )
            }
        } else {
            format!(
                "Rust code quality violations detected ({} issue(s)): {}",
                findings.len(),
                findings
                    .iter()
                    .map(|f| format!("{}: {} in {}", f.rule_id, f.description, f.file_path))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        Ok(RustSkillsReport {
            is_idiomatic,
            findings,
            rules_evaluated_count: engine::RULES.len(),
            categories_evaluated: engine::categories(),
            summary,
            measurement: RustSkillsMeasurement::Evaluated { rust_files },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detects_unwrap_in_prod() {
        let temp_dir = std::env::temp_dir();
        let guard = RustLanguagePolicy::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 601,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("/tmp"),
                crate::git_manager::Uncloned::TestFixture,
            ),
            diff_content: "+++ b/src/handler.rs\n+ let token = parse_header().unwrap();"
                .to_string(),
            changed_files: vec!["src/handler.rs".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_rust_quality(&temp_dir, &diff_ctx)
            .expect("eval");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].rule_id, "err-no-unwrap-prod");
    }

    #[test]
    fn test_detects_ref_string_param() {
        let temp_dir = std::env::temp_dir();
        let guard = RustLanguagePolicy::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 602,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(std::path::PathBuf::from("/tmp"), crate::git_manager::Uncloned::TestFixture),
            diff_content: "+++ b/src/service.rs\n+ pub fn process_name(name: &String) { println!(\"{}\", name); }".to_string(),
            changed_files: vec!["src/service.rs".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_rust_quality(&temp_dir, &diff_ctx)
            .expect("eval");
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].rule_id, "own-slice-over-vec");
    }

    #[test]
    fn test_detects_unsafe_without_safety_comment() {
        let temp_dir = std::env::temp_dir();
        let guard = RustLanguagePolicy::new();
        let diff_ctx = PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 603,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: crate::git_manager::SubjectRoot::asserted(
                std::path::PathBuf::from("/tmp"),
                crate::git_manager::Uncloned::TestFixture,
            ),
            diff_content: "+++ b/src/ffi.rs\n+ let ptr = unsafe { get_raw() };".to_string(),
            changed_files: vec!["src/ffi.rs".to_string()],
            is_incremental: false,
        };

        let report = guard
            .evaluate_rust_quality(&temp_dir, &diff_ctx)
            .expect("eval");
        assert!(!report.is_idiomatic);
        assert_eq!(report.findings[0].rule_id, "unsafe-safety-comment");
    }
}
