use crate::ratchet::facade::Signoff;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod signature_scanner;
pub use signature_scanner::{AbiScan, BreakingAbiFinding, SignatureScanner};

/// The gate this report publishes under, so an unmeasured layout is recorded
/// against the same id the scorecard renders.
pub const SEMANTIC_ABI_GATE_ID: &str = "semantic_abi_status";

/// Where a human records that a public signature change was intended.
pub const ABI_SIGNOFF_PATH: &str = ".anvil/baselines/semantic-abi.signoff.json";

/// The key a signoff names a finding by.
///
/// Symbol and file, never the line: a signed-off change that moves down its
/// file has not become a different decision, and a key that says otherwise
/// would expire for the wrong reason.
fn abi_key(f: &BreakingAbiFinding) -> String {
    format!("{}@{}", f.symbol_name, f.file_path)
}

/// What no diff-reading gate can answer, said once so every sentence below says
/// the same thing.
///
/// Struct memory layout is not derivable from diff text at any effort. A
/// `repr(Rust)` type has no guaranteed layout to be stable in the first place --
/// the compiler may reorder its fields and the Reference promises only
/// alignment, size-multiple-of-alignment and non-overlap -- so the claim is
/// meaningful only for `#[repr(C)]`, `#[repr(transparent)]` and integer reprs,
/// and computing it for those needs rustc (`-Z print-type-sizes`,
/// `offset_of!`) or DWARF out of a compiled artifact (`abidiff`). This gate has
/// neither, and says so instead of implying the layouts were checked.
const LAYOUT_DISCLAIMER: &str = "struct memory layout is not computed: no compiled artifact or type layout is available to \
     this gate, and only `#[repr(C)]`-family types have a layout to be stable at all";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticAbiReport {
    /// Whether the public function signatures this gate could compare are
    /// backward-compatible. It is not a verdict on layout, which was never
    /// measured -- see `status`.
    pub is_abi_stable: bool,
    pub breaking_findings: Vec<BreakingAbiFinding>,
    pub summary: String,
    /// The verdict, decided here and published unchanged.
    ///
    /// `slo_canary_guard` and `trace_context_guard` are the precedent: a gate
    /// that can distinguish "compared and clean" from "nothing to compare"
    /// cannot express the difference through a `bool`, and the evaluator
    /// rebuilding a two-valued status from one would erase it.
    pub status: GateStatus,
}

pub struct SemanticAbiRatchet {
    scanner: SignatureScanner,
}

impl Default for SemanticAbiRatchet {
    fn default() -> Self {
        Self::new()
    }
}

impl SemanticAbiRatchet {
    pub fn new() -> Self {
        let scanner = SignatureScanner::new();
        Self { scanner }
    }

    /// Deterministic evaluation of public function signature stability.
    ///
    /// The oracle for this gate is `cargo-semver-checks`, which builds rustdoc
    /// JSON for two revisions and runs 245 lints over the pair. This reads a
    /// unified diff and nothing else: it can see the declarations the change
    /// writes, and it cannot resolve a module path, a type, a trait impl or a
    /// layout. Every sentence it publishes is scoped to that.
    pub fn evaluate_abi_stability(
        &self,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
    ) -> Result<SemanticAbiReport> {
        info!(
            "Running SemanticAbiRatchet (Public Function Signature Stability Gate) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut scan = self.scanner.scan_abi_diff(&diff_ctx.diff_content);

        // A deliberate signature change is still a signature change; what a
        // signoff records is that someone decided to make it. Until this
        // existed the gate had no such path -- no allowlist, no waiver, no
        // baseline -- so the only ways past a considered API change were to
        // revert it or to leave the gate red.
        //
        // The entry expires on its own. This gate compares the two sides of a
        // diff rather than a stored baseline, so once the change merges the
        // diff no longer carries it and the key matches nothing. A key still
        // listed after that is inert and should be deleted; it is not doing
        // anything.
        let signoff = std::fs::read(repo_dir.join(ABI_SIGNOFF_PATH))
            .ok()
            .and_then(|b| Signoff::parse(&b).ok())
            .unwrap_or_default();
        let signed: Vec<String> = scan
            .findings
            .iter()
            .filter(|f| signoff.covers(SEMANTIC_ABI_GATE_ID, &abi_key(f)))
            .map(abi_key)
            .collect();
        scan.findings
            .retain(|f| !signoff.covers(SEMANTIC_ABI_GATE_ID, &abi_key(f)));
        if !signed.is_empty() {
            info!(
                "SemanticAbiRatchet: {} signed-off change(s): {}",
                signed.len(),
                signed.join(", ")
            );
        }

        let is_abi_stable = scan.findings.is_empty();

        let unpaired = if scan.unpaired_names == 0 {
            String::new()
        } else {
            format!(
                "; {} name(s) were declared on both sides more than once or across several lines, \
                 so their signatures were not compared",
                scan.unpaired_names
            )
        };

        let (status, summary) = if !is_abi_stable {
            let summary = format!(
                "❌ FAILED ({} breaking public function change(s): {}). {LAYOUT_DISCLAIMER}.",
                scan.findings.len(),
                scan.findings
                    .iter()
                    .map(|f| format!("{} {} at {}", f.change_kind, f.symbol_name, f.file_path))
                    .collect::<Vec<_>>()
                    .join("; ")
            );
            (GateStatus::Failed(summary.clone()), summary)
        } else if !scan.layout_files.is_empty() {
            // A `#[repr(...)]` line is the one case where the unmeasured half of
            // this gate's claim decides the answer, and a pass here would be the
            // gate's own defect restated: silence read as evidence. `NotMeasured`
            // makes no accusation and still withholds merge-queue admission
            // through `unmeasured_gates` (invariant I1).
            // "add or remove", not "change": what was measured is that a
            // `#[repr(...)]` line is present on one side of the diff and not
            // the other. A line present identically on both sides was moved and
            // no longer counts, but a brand-new `#[repr(C)]` type does -- it
            // adds a repr line, and this gate cannot tell that from an existing
            // type gaining one.
            let reason = format!(
                "{} of the diff's file(s) add or remove a `#[repr(...)]` line, and {LAYOUT_DISCLAIMER}",
                scan.layout_files.len()
            );
            (
                GateStatus::NotMeasured {
                    gate_id: SEMANTIC_ABI_GATE_ID.to_string(),
                    reason: reason.clone(),
                },
                format!("➖ NOT MEASURED ({reason})."),
            )
        } else {
            (
                GateStatus::Passed,
                format!(
                    "✅ PASSED ({} public function declaration(s) read; none removed without being \
                     re-added and no compared signature changed{unpaired}). {LAYOUT_DISCLAIMER}.",
                    scan.declarations_read
                ),
            )
        };

        Ok(SemanticAbiReport {
            is_abi_stable,
            breaking_findings: scan.findings,
            summary,
            status,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(diff: &str) -> PrDiffContext {
        PrDiffContext {
            repo: "oyatie/anvil".to_string(),
            pr_number: 100,
            base_branch: "main".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: diff.to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        }
    }

    fn run(diff: &str) -> SemanticAbiReport {
        SemanticAbiRatchet::new()
            .evaluate_abi_stability(Path::new("."), &ctx(diff))
            .unwrap()
    }

    #[test]
    fn test_abi_ratchet_nominal() {
        let rep = run(
            "diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n+pub fn get_version() -> &'static str { \"1.0\" }\n",
        );
        assert!(rep.is_abi_stable);
        assert!(matches!(rep.status, GateStatus::Passed));
    }

    #[test]
    fn every_summary_says_the_layout_was_not_computed() {
        // Three verdicts, one disclosure: the sentence a reviewer sees must never
        // imply that layouts were checked, whichever way the gate decided.
        let clean = run(
            "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n+pub fn added() {}\n",
        );
        let broken = run(
            "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-pub fn gone() {}\n",
        );
        let layout = run(
            "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n+#[repr(C)]\n",
        );
        for report in [&clean, &broken, &layout] {
            assert!(
                report.summary.contains("layout is not computed"),
                "{}",
                report.summary
            );
        }
        assert!(matches!(broken.status, GateStatus::Failed(_)));
        assert!(matches!(layout.status, GateStatus::NotMeasured { .. }));
    }
}
