//! Local inner-loop probe — the gate that graded a commit message it wrote.
//!
//! # What was here
//!
//! `evaluate_local_probe` passed the hardcoded literal `"feat: update codebase"`
//! to a validator whose entire conventional-commit check was
//! `commit_msg.starts_with("feat")`. `PrDiffContext` carries no commit message,
//! so that half was a constant answering a constant: it could not fail, and its
//! green described a string in this file rather than the pull request. The check
//! was also wrong about the specification it named — Conventional Commits 1.0.0
//! requires `<type>[(scope)][!]: <description>`, so `feat`, `feat:` and
//! `feature: x` are all invalid and `starts_with("feat")` accepted all three.
//!
//! `latency_ms: 18` was a literal too, and the module's own test asserted
//! `rep.latency_ms < 100` — an assertion over a constant, which holds for
//! exactly as long as the constant does.
//!
//! The published title claimed AST linting. There is no AST here and no parser
//! crate is a dependency; `syn::parse_file` needs a whole valid file, and the
//! added lines of a unified diff are not one.
//!
//! # What is here now
//!
//! Commit messages are obtainable — `git log <base>..<head>` in the clone the
//! pipeline already has, or GitHub's pull-request commits endpoint — so the
//! caller reads them and passes them in. The validator judges them against the
//! real grammar, skipping the subjects git generates the way commitlint's
//! `defaultIgnores` does.
//!
//! With no commit subject to judge, the conventional-commit half reports
//! [`GateStatus::NotMeasured`] naming what is missing. The secret scan is
//! unaffected: it reads the real diff and can turn this gate red on its own,
//! which is the two-half shape `hermetic_build` already uses.
//!
//! `latency_ms` is now the elapsed time of this call.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Instant;
use tracing::info;

use crate::git_manager::PrDiffContext;
use crate::pre_merge_guard::report::GateStatus;

pub mod fast_validator;
pub use fast_validator::{FastValidator, ProbeFinding};

/// Matches the `PreMergeCertificationReport` field name, so `unmeasured_gates`
/// names a gate a reader can look up in the fidelity registry.
const GATE_ID: &str = "local_probe_status";

const NO_COMMIT_MESSAGE_SOURCE: &str = "no commit message reached this gate: `PrDiffContext` carries only the diff, and \
     no subject survived the merge/fixup subjects commitlint ignores, so conventional \
     commit hygiene was never judged. The staged-diff secret scan did run.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalProbeReport {
    pub status: GateStatus,
    /// Whether every check that ran found nothing. False while unmeasured: a
    /// convention nobody checked is not a convention that was met.
    pub is_valid: bool,
    /// Elapsed wall-clock time of this call, in milliseconds.
    pub latency_ms: u64,
    pub findings: Vec<ProbeFinding>,
    pub summary: String,
}

pub struct LocalInnerLoopProbe {
    validator: FastValidator,
}

impl Default for LocalInnerLoopProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalInnerLoopProbe {
    pub fn new() -> Self {
        let validator = FastValidator::new();
        Self { validator }
    }

    /// Runs the pre-commit checks a `commit-msg` hook would run, over the commit
    /// subjects of this pull request and its diff.
    ///
    /// `commit_subjects` is what `git log <base>..<head>` returned. An empty
    /// slice means no commit source was available, not that the pull request has
    /// no commits.
    pub fn evaluate_local_probe(
        &self,
        _repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        commit_subjects: &[String],
    ) -> Result<LocalProbeReport> {
        let started = Instant::now();
        info!(
            "Running LocalInnerLoopProbe (pre-commit conventional-commit and secret checks) on {}#{}...",
            diff_ctx.repo, diff_ctx.pr_number
        );

        let mut findings: Vec<ProbeFinding> = commit_subjects
            .iter()
            .filter_map(|s| self.validator.check_commit_header(s))
            .collect();
        let judged_a_subject = !findings.is_empty();
        findings.push(self.validator.scan_staged_diff(&diff_ctx.diff_content));

        let failed: Vec<&ProbeFinding> = findings.iter().filter(|f| !f.is_valid).collect();
        let (status, is_valid, summary) = if !failed.is_empty() {
            let summary = format!(
                "{} pre-commit violation(s): {}",
                failed.len(),
                failed
                    .iter()
                    .map(|f| f.message.clone())
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            (GateStatus::Failed(summary.clone()), false, summary)
        } else if judged_a_subject {
            let summary = format!(
                "{} commit subject(s) conform to Conventional Commits 1.0.0; no credential \
                 prefix in the staged diff.",
                findings.len() - 1
            );
            (GateStatus::Passed, true, summary)
        } else {
            (
                GateStatus::NotMeasured {
                    gate_id: GATE_ID.to_string(),
                    reason: NO_COMMIT_MESSAGE_SOURCE.to_string(),
                },
                false,
                NO_COMMIT_MESSAGE_SOURCE.to_string(),
            )
        };

        Ok(LocalProbeReport {
            status,
            is_valid,
            latency_ms: started.elapsed().as_millis() as u64,
            findings,
            summary,
        })
    }
}

/// The rule harness, run over a staged diff, as probe findings.
///
/// This is `Harness::run`'s first production consumer. The harness -- `Rule`,
/// `Fixture`, `Corpus`, the codemod in `apply` -- was complete, tested and
/// called by nothing, which made it the largest built-and-unused subsystem in
/// the tree. Four rules were registered and none of them examined a real
/// change.
///
/// Running it at the probe rather than at certification is deliberate. The
/// prevention ladder puts a defect's cost at the rung that catches it, and
/// `secret_on_added_line` is worth far more before the commit exists than on a
/// scorecard afterwards. The pre-commit hook already invokes `anvil probe`.
///
/// A withheld rule is reported, never silently dropped: `Harness::run` inserts
/// an entry for every registered rule on every path, and a rule that could not
/// run has not passed. That is the same distinction the probe's own commit
/// header check needed.
pub fn harness_findings(staged_diff: &str, message: Option<&str>) -> Vec<ProbeFinding> {
    use crate::harness::corpus::Corpus;
    use crate::harness::{Evaluated, rules};

    let paths: Vec<String> = crate::git_manager::diff_context::diffs_by_path(staged_diff)
        .into_iter()
        .map(|fd| fd.path)
        .collect();
    let path_refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    let mut corpus = Corpus::of_diff(&path_refs, staged_diff);
    // The subject rule is REGISTERED whether or not a message exists. With one
    // it is measured; without one the harness withholds it as `InputsAbsent`
    // and says so. Leaving it unregistered would have been indistinguishable
    // from a rule that ran and found nothing -- which is precisely the
    // confusion the probe's hardcoded commit literal created.
    if let Some(m) = message {
        corpus = corpus.with_commits(vec![m.to_string()]);
    }

    // `rules::registered()` is the one registration point. A second list here
    // would drift from it silently: a rule added there and forgotten here is
    // invisible, which is the shape of every defect this probe now catches.
    let harness = rules::registered();

    // Every registered rule is declared here. The probe is not spec-driven:
    // there is no tenant spec at pre-commit time, and a rule silently
    // undeclared would be indistinguishable from a rule that passed.
    let run = harness.run(&corpus, &|_| true);

    let mut out = Vec::new();
    for (id, evaluated) in &run.per_rule {
        match evaluated {
            Evaluated::Measured { findings, .. } if findings.is_empty() => out.push(ProbeFinding {
                check_name: format!("harness:{id}"),
                is_valid: true,
                message: "measured, nothing found".to_string(),
            }),
            Evaluated::Measured { findings, .. } => {
                for f in findings {
                    out.push(ProbeFinding {
                        check_name: format!("harness:{id}"),
                        is_valid: false,
                        message: format!("{}: {}", f.subject, f.detail),
                    });
                }
            }
            // Not a pass. A rule whose inputs are absent measured nothing, and
            // one whose fixture failed cannot be trusted to have measured
            // anything -- reporting either as clean is the defect the fixture
            // mechanism exists to prevent.
            Evaluated::Withheld(w) => out.push(ProbeFinding {
                check_name: format!("harness:{id}"),
                is_valid: true,
                message: format!("NOT MEASURED ({w:?})"),
            }),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(diff: &str) -> PrDiffContext {
        PrDiffContext {
            repo: "oyatie/oyatie".to_string(),
            pr_number: 100,
            base_branch: "dev".to_string(),
            base_sha: "aaa".to_string(),
            head_sha: "bbb".to_string(),
            diff_content: diff.to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: std::path::PathBuf::from("."),
            is_incremental: false,
            previous_head_sha: None,
        }
    }

    #[test]
    fn test_local_probe_without_commit_subjects_is_unmeasured() {
        let rep = LocalInnerLoopProbe::new()
            .evaluate_local_probe(Path::new("."), &ctx("+ fn local() {}"), &[])
            .unwrap();

        assert_eq!(rep.status.unmeasured_gate_id(), Some(GATE_ID));
        assert!(!rep.is_valid, "an unjudged convention is not a met one");
    }

    #[test]
    fn test_local_probe_passes_real_conventional_subjects() {
        let rep = LocalInnerLoopProbe::new()
            .evaluate_local_probe(
                Path::new("."),
                &ctx("+ fn local() {}"),
                &["feat(probe): read real subjects".to_string()],
            )
            .unwrap();

        assert!(matches!(rep.status, GateStatus::Passed));
        assert!(rep.is_valid);
    }
}
