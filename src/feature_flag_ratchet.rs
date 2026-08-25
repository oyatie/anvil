//! Feature flag lifecycle — the gate whose vocabulary no flag system uses.
//!
//! # What was here
//!
//! Three rules, none of which any pull request could trip.
//!
//! `@deprecated_flag` and `@stale_flag` are conventions of no flag system that
//! could be identified. LaunchDarkly computes staleness on its own backend from
//! flag age and evaluation status, and `ld-find-code-refs` searches source for
//! the flag *key* between quote delimiters, reading no annotation. Unleash
//! carries `stale` as a boolean on the flag object, set through the admin API or
//! by a per-flag-type expected lifetime. Statsig marks a temporary gate stale
//! from its rollout state. OpenFeature is an evaluation API specification and
//! defines neither staleness nor any source annotation. Uber's `piranha` is
//! given the flag key and its expected behaviour *by the operator* — staleness
//! is decided outside the tool — and then deletes the dead branch by tree-sitter
//! AST rewriting. Chromium, the nearest real "flags expire" system, keeps expiry
//! in `chrome/browser/flag-metadata.json`, not in a comment. Both tokens
//! occurred exactly twice in this repository: in the regex, and in this module's
//! own fixture.
//!
//! `EXPIRATION:\s*202[0-5]` stopped at 2025, so in 2026 it had aged out of its
//! own window: an annotation written today could not match it. It occurred once,
//! in the regex.
//!
//! The dead-branch rule required the literal source `if true && …
//! is_feature_enabled(…)`, which rustc and clippy both object to, so it appeared
//! nowhere.
//!
//! With every rule unmatchable, `violations` was empty on every input and the
//! gate published "zero stale or permanent toggle bloat detected" — a green no
//! pull request in the last twelve months could have turned red.
//!
//! # What is here now
//!
//! The honest half is kept and promoted: [`FeatureFlagRatchet::scan_flag_references`]
//! locates a toggle by its **key at the call site**, which is what
//! `ld-find-code-refs` does and is the seam a real LaunchDarkly, Unleash or
//! Statsig lookup plugs into.
//!
//! What a code scan cannot know is whether a key it found is stale — that is a
//! fact the flag-management system owns. Anvil talks to none, so the gate reads
//! a ledger the repository under review may keep, the shape `debt_shrink_guard`
//! already uses for `REORG-DRAIN.md`. With no ledger, or with no flag reference
//! in the diff, the gate reports [`GateStatus::NotMeasured`] naming what is
//! missing — not `Passed`, and not `Failed`, which would accuse every pull
//! request that touches a toggle.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

/// Matches the `PreMergeCertificationReport` field name, so `unmeasured_gates`
/// names a gate a reader can look up in the fidelity registry.
const GATE_ID: &str = "feature_flag_status";

/// Where a repository may record which flag keys are stale. Anvil's own
/// convention, not an industry one: every flag system keeps this in its backend,
/// and a ledger is what a repository with no such backend can offer instead.
const LEDGER_PATHS: &[&str] = &["governance/STALE-FLAGS.md", "STALE-FLAGS.md"];

const NO_LIFECYCLE_SOURCE: &str = "no flag lifecycle source: no LaunchDarkly, Unleash or Statsig API is queried and \
     neither governance/STALE-FLAGS.md nor STALE-FLAGS.md exists in the repository \
     under review, so whether any toggle found here is stale was never looked up";

const NO_FLAG_REFERENCE_IN_SCOPE: &str = "no flag reference in the added lines: nothing in this change reads a toggle by \
     a key written at the call site, so no flag lifecycle was evaluated; an empty \
     scope is not a retired one";

/// One toggle read at a call site, located by its key — the `ld-find-code-refs`
/// unit of evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlagReference {
    pub file_path: String,
    pub flag_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureFlagViolation {
    pub file_path: String,
    pub flag_name: String,
    pub issue_type: String,
    pub description: String,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlagReport {
    pub status: GateStatus,
    /// Whether a lifecycle was actually looked up AND found nothing stale. False
    /// while unmeasured: a flag nobody asked about is not a retired one.
    pub is_clean: bool,
    pub flags_scanned_count: usize,
    pub violations: Vec<FeatureFlagViolation>,
    pub summary: String,
}

pub struct FeatureFlagRatchet;

impl Default for FeatureFlagRatchet {
    fn default() -> Self {
        Self::new()
    }
}

impl FeatureFlagRatchet {
    pub fn new() -> Self {
        Self
    }

    /// Every toggle the added lines read by a key written at the call site.
    ///
    /// A key given as a variable, a constant or an enum is invisible here, as it
    /// is to `ld-find-code-refs`, which matches the key text between quote
    /// delimiters. The call names are a fixed list, so a wrapper spelled any
    /// other way is missed; the registry gap says so.
    pub fn scan_flag_references(diff_content: &str) -> Vec<FlagReference> {
        let flag_usage_re = Regex::new(
            r#"(?i)(?:is_feature_enabled|feature_flag|useFeatureFlag|flags\.get)\s*\(\s*["']([^"']+)["']"#,
        )
        .expect("the flag call-site pattern is a compile-time constant");

        let mut refs = Vec::new();
        // `None` until the diff names a file. It used to be `String::new()`,
        // so a `+` line arriving before any `+++ b/` header produced a
        // reference whose path was the empty string:
        //
        //     flag refs: [("", "new_billing")]
        //
        // A flag reference that cannot say which file it is in is not one a
        // reader can act on, and it is not evidence the flag is referenced
        // anywhere -- which is what this scan exists to establish.
        let mut current_file: Option<&str> = None;

        for line in diff_content.lines() {
            if let Some(stripped) = line.strip_prefix("+++ b/") {
                current_file = Some(stripped.trim());
                continue;
            }
            if !line.starts_with('+') || line.starts_with("+++") {
                continue;
            }
            let Some(path) = current_file else {
                continue;
            };
            for caps in flag_usage_re.captures_iter(line[1..].trim()) {
                refs.push(FlagReference {
                    file_path: path.to_string(),
                    flag_key: caps[1].to_string(),
                });
            }
        }
        refs
    }

    /// Judges caller-supplied references against a caller-supplied ledger.
    ///
    /// This is the seam: a real flag-management API returns the same thing the
    /// ledger does — the set of keys that are stale — and plugs in here without
    /// touching the caller.
    pub fn evaluate_flag_lifecycle(
        &self,
        refs: &[FlagReference],
        stale_flag_ledger: &str,
    ) -> FeatureFlagReport {
        if stale_flag_ledger.trim().is_empty() {
            return Self::unmeasured(NO_LIFECYCLE_SOURCE, refs.len());
        }
        if refs.is_empty() {
            return Self::unmeasured(NO_FLAG_REFERENCE_IN_SCOPE, 0);
        }

        let violations: Vec<FeatureFlagViolation> = refs
            .iter()
            .filter(|r| Self::ledger_records_stale(stale_flag_ledger, &r.flag_key))
            .map(|r| FeatureFlagViolation {
                file_path: r.file_path.clone(),
                flag_name: r.flag_key.clone(),
                issue_type: "STALE_FLAG_REFERENCED".to_string(),
                description: format!(
                    "This change reads `{}`, which the stale-flag ledger records as retired.",
                    r.flag_key
                ),
                recommendation:
                    "Delete the toggle check and the branch it guards, then drop the key from \
                     the ledger."
                        .to_string(),
            })
            .collect();

        let is_clean = violations.is_empty();
        let summary = if is_clean {
            format!(
                "Flag lifecycle checked against the stale-flag ledger: {} reference(s) scanned, \
                 none recorded stale.",
                refs.len()
            )
        } else {
            format!(
                "{} reference(s) to a flag the ledger records as stale: {}",
                violations.len(),
                violations
                    .iter()
                    .map(|v| format!("{} ({})", v.flag_name, v.file_path))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };

        FeatureFlagReport {
            // A stale toggle is debt, not a defect: LaunchDarkly and Unleash
            // both surface it for cleanup rather than blocking a release, so
            // this warns instead of refusing the merge.
            status: if is_clean {
                GateStatus::Passed
            } else {
                GateStatus::Warning(summary.clone())
            },
            is_clean,
            flags_scanned_count: refs.len(),
            violations,
            summary,
        }
    }

    /// The review pipeline's entry point: scan the change for toggles, then ask
    /// the repository's own ledger whether any of them is stale.
    pub fn evaluate_feature_flags(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<FeatureFlagReport> {
        info!(
            "Running FeatureFlagRatchet on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let ledger = LEDGER_PATHS
            .iter()
            .map(|p| repo_dir.join(p))
            .find(|p| p.is_file())
            .and_then(|p| std::fs::read_to_string(p).ok())
            .unwrap_or_default();

        let refs = Self::scan_flag_references(&diff_ctx.diff_content);
        Ok(self.evaluate_flag_lifecycle(&refs, &ledger))
    }

    /// A ledger line records a key when it names it between backticks, so a key
    /// mentioned in prose is not mistaken for a record and a substring of a
    /// longer key does not match.
    fn ledger_records_stale(ledger: &str, flag_key: &str) -> bool {
        ledger.contains(&format!("`{}`", flag_key))
    }

    fn unmeasured(reason: &str, flags_scanned_count: usize) -> FeatureFlagReport {
        FeatureFlagReport {
            status: GateStatus::NotMeasured {
                gate_id: GATE_ID.to_string(),
                reason: reason.to_string(),
            },
            is_clean: false,
            flags_scanned_count,
            violations: Vec::new(),
            summary: reason.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff_ctx(diff: &str) -> PrDiffContext {
        PrDiffContext {
            repo: "oyatie/console".to_string(),
            pr_number: 301,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            previous_head_sha: None,
            repo_working_dir: std::path::PathBuf::from("/tmp"),
            diff_content: diff.to_string(),
            changed_files: vec!["src/features.ts".to_string()],
            is_incremental: false,
        }
    }

    #[test]
    fn test_flag_usage_without_a_ledger_is_not_a_pass() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let report = FeatureFlagRatchet::new()
            .evaluate_feature_flags(
                temp_dir.path(),
                &diff_ctx(
                    "+++ b/src/features.ts\n+ if (is_feature_enabled('new_billing_v2')) { doNew(); }",
                ),
            )
            .expect("eval");

        assert_eq!(report.status.unmeasured_gate_id(), Some(GATE_ID));
        assert_eq!(report.flags_scanned_count, 1);
        assert!(!report.is_clean);
    }

    #[test]
    fn test_stale_ledger_entry_is_reported_against_the_reference() {
        let report = FeatureFlagRatchet::new().evaluate_flag_lifecycle(
            &[FlagReference {
                file_path: "src/features.ts".to_string(),
                flag_key: "new_billing_v2".to_string(),
            }],
            "- `new_billing_v2`\n",
        );

        assert!(!report.is_clean);
        assert_eq!(report.violations[0].issue_type, "STALE_FLAG_REFERENCED");
    }

    #[test]
    fn test_a_key_mentioned_in_prose_is_not_a_ledger_record() {
        let report = FeatureFlagRatchet::new().evaluate_flag_lifecycle(
            &[FlagReference {
                file_path: "src/features.ts".to_string(),
                flag_key: "new_billing".to_string(),
            }],
            "We are keeping new_billing for now; `new_billing_v2` is stale.\n",
        );

        assert!(
            report.is_clean,
            "`new_billing` is neither backticked nor the whole of the backticked key"
        );
    }
}
