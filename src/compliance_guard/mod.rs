use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

pub mod engine;
pub mod registry;
pub mod upstream_sync;

pub use engine::{RegulatoryEngine, StatutoryViolation};
pub use upstream_sync::UpstreamRegulatorySync;

use crate::git_manager::PrDiffContext;

/// Where a repository under review may ship its own regulatory rules, relative
/// to its working tree. One JSON rule per file, additive only: see
/// [`upstream_sync::UpstreamRegulatorySync::load_rule_pack`].
const RULE_PACK_DIR: &str = "policies/regulatory";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceGuardReport {
    pub is_compliant: bool,
    pub violations: Vec<StatutoryViolation>,
    pub evaluation_date: String,
    /// One line per rule that was actually enforceable on `evaluation_date`,
    /// derived from those rules. Previously a hardcoded `vec!` advertising
    /// roughly seventeen statutes across five jurisdictions, of which four
    /// rules were implemented.
    pub statutes_evaluated: Vec<String>,
    /// Rules that were in force on `evaluation_date` *and* carry a pattern the
    /// engine can evaluate. A rule with `pattern_regex: None` can never fire,
    /// so counting it inflated the sentence this number appears in.
    pub active_rules_count: usize,
    /// Rules hot-loaded from the repository's own rule pack. The sync path had
    /// zero callers, so the registry's "dynamic" half described unreachable
    /// code.
    pub rules_loaded_from_pack: usize,
    /// One sentence per rule-pack file that contributed nothing, and why. A
    /// pack that failed to load must not read the same as no pack at all.
    pub pack_rules_rejected: Vec<String>,
    /// Matches waived by a line naming the rule through
    /// [`engine::SUPPRESSION_MARKER`]. Published, so silencing a statute costs
    /// a visible line in the gate's own output.
    pub suppressed_matches: usize,
    pub summary: String,
}

pub struct ComplianceGuard {
    engine: RegulatoryEngine,
    sync: UpstreamRegulatorySync,
}

impl Default for ComplianceGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl ComplianceGuard {
    pub fn new() -> Self {
        Self {
            engine: RegulatoryEngine::new(),
            sync: UpstreamRegulatorySync::new(),
        }
    }

    /// Reads the regulatory rule pack a repository ships, as JSON files.
    ///
    /// This had zero callers. `evaluate_compliance_at` now calls it on every
    /// run against the repository under review, which is what makes the
    /// registry dynamic rather than merely described as such. Anvil has no HTTP
    /// client, so a rule pack arrives as files in the tree it is already
    /// holding -- Semgrep's `--config ./rules/` shape rather than its registry
    /// fetch. There is no staleness notion here and no signature on the pack;
    /// Grype's `db.max-allowed-built-age` is the thing this is not. The pack is
    /// returned rather than stored: it belongs to the repository that shipped
    /// it and to no other.
    pub fn read_rule_pack(&self, dir: &Path) -> Result<upstream_sync::RulePack> {
        self.sync.load_rule_pack(dir)
    }

    /// Evaluates a diff against every rule in force today.
    ///
    /// Today comes from the clock. It used to be `"2026-08-19"`, a literal
    /// commented "Canonical platform time", so every `effective_date` and
    /// `sunset_date` on every rule was compared against a date that had stopped
    /// moving and the temporal machinery could not change its answer.
    ///
    /// Unfreezing it makes the verdict time-dependent: a rule whose
    /// `effective_date` falls between two runs turns a green pull request red
    /// with no code change. That is correct rather than a footgun -- a statute
    /// takes effect on its date whether or not a scanner noticed, and a gate
    /// asserting compliance with a legal regime that has been superseded is
    /// making exactly the unsupported claim this file exists to remove. The
    /// softening is the one the registry already models and no upstream scanner
    /// implements: `grace_period_until` demotes a newly effective rule to
    /// `ADVISORY`, which does not block, so a rule can land in monitor mode
    /// before it lands in blocking mode. `evaluation_date` is published on the
    /// report so a reader can tell the two runs apart.
    ///
    /// Tests pin an explicit date through [`Self::evaluate_compliance_at`]; the
    /// clock is exercised in exactly one test.
    pub fn evaluate_compliance(&self, diff_ctx: &PrDiffContext) -> Result<ComplianceGuardReport> {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        self.evaluate_compliance_at(diff_ctx, &today)
    }

    /// [`Self::evaluate_compliance`] with the evaluation date supplied.
    pub fn evaluate_compliance_at(
        &self,
        diff_ctx: &PrDiffContext,
        current_date: &str,
    ) -> Result<ComplianceGuardReport> {
        info!(
            "Running Dynamic Regulatory Compliance Guard (Temporal Date: {}) on {}#{}...",
            current_date, diff_ctx.repo, diff_ctx.pr_number
        );

        let pack = self.read_rule_pack(&diff_ctx.repo_working_dir.join(RULE_PACK_DIR))?;
        let rules_loaded_from_pack = pack.rules.len();

        let enforceable_rules = self.sync.enforceable_rules(current_date, &pack.rules);
        let active_rules_count = enforceable_rules.len();
        let (violations, suppressed_matches) =
            self.engine.scan_diff(diff_ctx, &enforceable_rules)?;

        // Derived, not declared. The previous `vec!` named roughly seventeen
        // statutes across five jurisdictions and was a constant, so it stayed
        // true of nothing.
        let statutes_evaluated: Vec<String> = enforceable_rules
            .iter()
            .map(|(r, _)| {
                format!(
                    "{} — {} [{:?}]",
                    r.statute_or_policy_name, r.citation, r.scope
                )
            })
            .collect();

        let has_blocking_violations = violations
            .iter()
            .any(|v| v.severity == "CRITICAL" || v.severity == "HIGH");
        let is_compliant = !has_blocking_violations;

        // Never silent: a waived match and a rejected pack file are the two ways
        // this gate can look clean without having looked.
        let caveats = format!(
            "{}{}",
            if suppressed_matches == 0 {
                String::new()
            } else {
                format!(" {suppressed_matches} match(es) were waived by a line naming the rule.")
            },
            if pack.rejected.is_empty() {
                String::new()
            } else {
                format!(
                    " {} rule-pack file(s) contributed nothing: {}.",
                    pack.rejected.len(),
                    pack.rejected.join("; ")
                )
            }
        );

        let summary = if is_compliant {
            format!(
                "No match against the {} rule(s) enforceable on {} ({} of them from this \
                 repository's own rule pack). This is a regex scan of added lines, not a legal \
                 opinion: only the statutes listed in `statutes_evaluated` were checked at \
                 all.{}",
                active_rules_count, current_date, rules_loaded_from_pack, caveats
            )
        } else {
            format!(
                "Regulatory & statutory violations detected ({} violation(s)) evaluating for {}: {}{}",
                violations.len(),
                current_date,
                violations
                    .iter()
                    .map(|v| format!("{}: {} [{}]", v.citation, v.title, v.line_snippet))
                    .collect::<Vec<_>>()
                    .join("; "),
                caveats
            )
        };

        Ok(ComplianceGuardReport {
            is_compliant,
            violations,
            evaluation_date: current_date.to_string(),
            statutes_evaluated,
            active_rules_count,
            rules_loaded_from_pack,
            pack_rules_rejected: pack.rejected,
            suppressed_matches,
            summary,
        })
    }
}
