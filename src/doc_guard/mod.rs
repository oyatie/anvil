use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tracing::{info, warn};

pub mod corpus_sync;
pub mod docs_as_code_guard;
pub mod frontmatter;
pub use docs_as_code_guard::{DocsAsCodeGuard, DocsAsCodeReport};
pub use frontmatter::{DocFrontmatter, FrontmatterValidator};

use crate::git_manager::PrDiffContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocParityEvaluation {
    pub is_doc_sufficient: bool,
    pub missing_doc_summary: Option<String>,
    #[serde(default)]
    pub doc_files_to_update: Vec<String>,
    #[serde(default)]
    pub suggested_adr_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocGuardReport {
    pub is_sufficient: bool,
    pub files_created_or_updated: Vec<String>,
    pub summary: String,
    /// Set when the guard could not obtain a judgement at all (spawn failure,
    /// timeout, unparseable response). Distinct from `is_sufficient: false`,
    /// which is a real adverse finding.
    ///
    /// Invariant I1: absent evidence is never a pass — but nor is it a
    /// fabricated accusation, so this maps to `GateStatus::Errored`.
    pub errored: Option<String>,
}

/// An xhigh-effort model call cannot reliably complete in 20 seconds, which is
/// what the previous limit was; every timeout became a silent pass.
const DOC_PARITY_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

pub struct DocGuard {
    agy_effort: String,
}

impl DocGuard {
    pub fn new(agy_effort: String) -> Self {
        Self { agy_effort }
    }

    /// Constructs a guard whose doc-parity judgement is supplied directly
    /// instead of being obtained by spawning the `agy` probe.
    ///
    /// SCAFFOLDING (`tdd/docguard-oracle-repair`): signature only, body left to
    /// the implementer.
    ///
    /// Both of the behaviours the specification asks for on the far side of the
    /// probe — issue #29's "a diff the probe judged insufficient does not yield
    /// a sufficient report", and issue #27's "the gate's summary must state the
    /// sync did not apply" — are only reachable through
    /// `ensure_documentation_parity` after a model has run. Without a seam, the
    /// only testable surface is a helper that nothing is obliged to call, and a
    /// suite pinning it can go green over an entry point that was never fixed.
    ///
    /// The shape here (a constructor taking the evaluation) is one of several
    /// that would serve; an injected trait object or a boxed async closure would
    /// do as well, and the implementer may substitute either. What the tests
    /// depend on is only that the public entry point can be driven with a known
    /// judgement and without a model.
    #[allow(unused_variables)]
    pub fn with_probe_override(agy_effort: String, evaluation: DocParityEvaluation) -> Self {
        todo!("supply the doc-parity judgement without spawning the agy probe")
    }

    /// Evaluates documentation parity, frontmatter compliance, and auto-generates any missing docs or ADRs.
    ///
    /// Published gate-count claims are owned by `corpus_sync`. That pass is
    /// mechanical and runs first. The LLM probe is not the authority for counts.
    pub async fn ensure_documentation_parity(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
    ) -> Result<DocGuardReport> {
        info!(
            "Running DocGuard documentation parity & frontmatter check on {}#{}...",
            repo, diff_ctx.pr_number
        );

        // Mechanical corpus sync first. Remaining drift must fail the gate
        // without being reported as AutoUpdated (the evaluator treats a
        // non-empty files list as AutoUpdated).
        let rewritten = match corpus_sync::sync_published_counts(
            repo,
            repo_dir,
            crate::pre_merge_guard::report::TOTAL_GATES,
        ) {
            Ok(sync) if !sync.remaining_drift.is_empty() => {
                return Ok(DocGuardReport {
                    errored: None,
                    is_sufficient: false,
                    files_created_or_updated: Vec::new(),
                    summary: format!(
                        "Published docs still disagree with TOTAL_GATES={}: {}",
                        crate::pre_merge_guard::report::TOTAL_GATES,
                        sync.remaining_drift.join("; ")
                    ),
                });
            }
            Ok(sync) => sync.rewritten,
            Err(e) => {
                return Ok(DocGuardReport {
                    errored: Some(e.to_string()),
                    is_sufficient: false,
                    files_created_or_updated: Vec::new(),
                    summary: format!("Could not make published docs honest: {e}"),
                });
            }
        };

        // Step 1: Validate frontmatters on all modified documentation and config files
        for file in &diff_ctx.changed_files {
            let file_full = repo_dir.join(file);
            if file_full.exists()
                && let Ok(content) = tokio::fs::read_to_string(&file_full).await
                && let Err(err) =
                    FrontmatterValidator::validate_doc_frontmatter(file, &content, repo_dir)
            {
                warn!("DocGuard frontmatter violation: {}", err);
                return Ok(DocGuardReport {
                    errored: None,
                    is_sufficient: false,
                    files_created_or_updated: Vec::new(),
                    summary: format!("❌ Frontmatter & SSOT validation failed: {}", err),
                });
            }
        }

        // Step 2: Analyze semantic documentation parity.
        //
        // A probe failure is reported as `errored`, not propagated: the rest of
        // the gate matrix must still run and the scorecard must still post.
        // It maps to GateStatus::Errored, which blocks (invariant I1) without
        // claiming the documentation is actually deficient.
        let eval = match self
            .evaluate_doc_parity(repo, repo_dir, diff_ctx, pr_title, pr_body)
            .await
        {
            Ok(e) => e,
            Err(e) => {
                warn!(
                    "Doc parity probe could not produce a judgement for {}#{}: {}",
                    repo, diff_ctx.pr_number, e
                );
                return Ok(DocGuardReport {
                    errored: Some(e.to_string()),
                    is_sufficient: false,
                    files_created_or_updated: Vec::new(),
                    summary: format!("Documentation parity could not be evaluated: {}", e),
                });
            }
        };

        if eval.is_doc_sufficient {
            info!(
                "Documentation parity is satisfied for {}#{}",
                repo, diff_ctx.pr_number
            );
            let summary = if rewritten.is_empty() {
                "Documentation and SSOT frontmatters satisfy the required fields and parity rules."
                    .to_string()
            } else {
                format!(
                    "Published docs rewritten to TOTAL_GATES={}: {}",
                    crate::pre_merge_guard::report::TOTAL_GATES,
                    rewritten.join(", ")
                )
            };
            return Ok(DocGuardReport {
                errored: None,
                is_sufficient: true,
                files_created_or_updated: rewritten,
                summary,
            });
        }

        info!(
            "Missing documentation identified for {}#{}: {:?}. Auto-generating documentation...",
            repo, diff_ctx.pr_number, eval.doc_files_to_update
        );

        // Step 3: Auto-generate missing documentation / ADRs in the workspace
        let mut updated_files = self
            .generate_and_write_docs(repo, repo_dir, diff_ctx, pr_title, pr_body, &eval)
            .await?;
        updated_files.extend(rewritten);

        let summary = format!(
            "Auto-generated documentation updates for: {}",
            updated_files.join(", ")
        );

        Ok(DocGuardReport {
            errored: None,
            is_sufficient: true,
            files_created_or_updated: updated_files,
            summary,
        })
    }

    /// Applies a `DocParityEvaluation` that judged the diff under-documented,
    /// and composes the report for it.
    ///
    /// SCAFFOLDING (`tdd/docguard-oracle-repair`): signature only, body left to
    /// the implementer. This exists because the tail of
    /// `ensure_documentation_parity` — the code issue #29 is about — is only
    /// reachable after the `agy` probe has run, and a test must never spawn the
    /// probe. The implementer moves that tail here, fixes it, and calls this
    /// from `ensure_documentation_parity`.
    #[allow(dead_code, unused_variables)]
    async fn apply_doc_parity_evaluation(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
        eval: &DocParityEvaluation,
    ) -> DocGuardReport {
        todo!("compose the report for a diff the probe judged under-documented")
    }

    async fn evaluate_doc_parity(
        &self,
        repo: &str,
        repo_dir: &Path,
        diff_ctx: &PrDiffContext,
        pr_title: &str,
        pr_body: &str,
    ) -> Result<DocParityEvaluation> {
        let changed_files_preview = if diff_ctx.changed_files.len() > 100 {
            format!(
                "{}\n- ... and {} more files",
                diff_ctx
                    .changed_files
                    .iter()
                    .take(100)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n- "),
                diff_ctx.changed_files.len() - 100
            )
        } else {
            diff_ctx.changed_files.join("\n- ")
        };

        let diff_content_bounded = if diff_ctx.diff_content.chars().count() > 50_000 {
            let truncated: String = diff_ctx.diff_content.chars().take(50_000).collect();
            format!("{truncated}\n\n[... remaining diff truncated for doc evaluation ...]")
        } else {
            diff_ctx.diff_content.clone()
        };

        let prompt = format!(
            r#####"You are Oyatie's Principal Documentation Architect. Evaluate whether this Pull Request on `{repo}` has sufficient documentation parity or if documentation must be updated.

## Pull Request Information:
- **Repository**: {repo}
- **PR Number**: #{pr_number}
- **Title**: {pr_title}
- **Description**: {pr_body}
- **Changed Files**:
- {changed_files}

## Evaluation Criteria:
1. **API / Public Interface Changes**: Are new public functions, types, routes, or CLI flags introduced without docstrings or `docs/reference/` updates?
2. **Architectural / Doctrine Shifts**: Does this change introduce a new architectural decision, storage pattern, cell boundary, or platform contract that requires an ADR (in `docs/decisions/` or `docs/design/`)?
3. **User-Facing / Config Changes**: Does `README.md`, `CLAUDE.md`, `AGENTS.md`, or runbooks need updating?
4. **Changelog**: Does `CHANGELOG.md` need a release note entry?

## Output Format:
Output strictly valid JSON matching this schema:
```json
{{
  "is_doc_sufficient": false,
  "missing_doc_summary": "Explanation of what documentation or ADR is missing",
  "doc_files_to_update": ["docs/reference/feature.md", "CHANGELOG.md"],
  "suggested_adr_title": null
}}
```

Note: If documentation is already sufficient, set `is_doc_sufficient: true`, `missing_doc_summary: null`, `doc_files_to_update: []`.

## Git Diff:
```diff
{diff_content}
```
"#####,
            repo = repo,
            pr_number = diff_ctx.pr_number,
            pr_title = pr_title,
            pr_body = if pr_body.is_empty() {
                "No description"
            } else {
                pr_body
            },
            changed_files = changed_files_preview,
            diff_content = diff_content_bounded
        );

        let target = format!("{}#{}", repo, diff_ctx.pr_number);
        let agy_effort = self.agy_effort.clone();
        let repo_dir_owned = repo_dir.to_path_buf();
        let prompt_clone = prompt.clone();

        crate::watchdog::PipelineWatchdog::run_with_watchdog(
            "DocGuardEvaluation",
            &target,
            std::time::Duration::from_secs(30),
            move || async move {
                let mut cmd = Command::new("agy");
                // Match the invocation form used by every other agy call site
                // (`--print <prompt> --effort <e>`); the previous
                // `prompt --raw` form was unique to this guard.
                cmd.args([
                    "--print",
                    &prompt_clone,
                    "--effort",
                    &agy_effort,
                    "--print-timeout",
                    &crate::exec::agy_print_timeout_arg(DOC_PARITY_PROBE_TIMEOUT),
                    // Required for agy to read the repository at all. Omitting it
                    // in the Phase 0a rewrite made every doc-parity probe fail
                    // with "permission check failed for command", which the
                    // fail-closed change then surfaced as a blocked gate --
                    // correctly, but for a reason this code introduced.
                    //
                    // This probe only READS, so a scoped read-only agy mode would
                    // be the right long-term fix; passing the blanket flag here
                    // widens the S5 surface by one more call site.
                    "--dangerously-skip-permissions",
                ]);
                // Run inside the repository under review. Previously unset, so
                // this probe executed in anvil's own working directory and
                // judged the wrong tree.
                cmd.current_dir(&repo_dir_owned);

                match crate::exec::run_bounded_for(
                    cmd,
                    DOC_PARITY_PROBE_TIMEOUT,
                    "doc parity probe",
                )
                .await
                {
                    Ok(output) if output.status.success() => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        if let Some(json_str) = extract_json_block(&stdout)
                            && let Ok(eval) = serde_json::from_str::<DocParityEvaluation>(&json_str)
                        {
                            return Ok(eval);
                        }
                        // Ran successfully but produced nothing parseable: we
                        // have no judgement, so we must not claim sufficiency.
                        anyhow::bail!(
                            "doc parity probe returned no parseable evaluation (stdout {} bytes)",
                            stdout.len()
                        )
                    }
                    Ok(output) => anyhow::bail!(
                        "doc parity probe exited with status {}: {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr).trim()
                    ),
                    // `run_bounded_for` already distinguishes "failed to run"
                    // from "timed out" in its message, and both stay errors.
                    Err(e) => Err(e),
                }
            },
            |err| {
                // No deterministic local fallback exists for doc parity, so the
                // watchdog path must report the failure rather than manufacture
                // a pass. This arm previously returned is_doc_sufficient: true,
                // which made gate 1 unfailable.
                Err(anyhow::anyhow!(
                    "doc parity probe supervision failed: {}",
                    err
                ))
            },
        )
        .await
    }

    async fn generate_and_write_docs(
        &self,
        _repo: &str,
        repo_dir: &Path,
        _diff_ctx: &PrDiffContext,
        _pr_title: &str,
        _pr_body: &str,
        eval: &DocParityEvaluation,
    ) -> Result<Vec<String>> {
        let mut updated = Vec::new();
        for file in &eval.doc_files_to_update {
            let path = repo_dir.join(file);
            if let Some(parent) = path.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if !path.exists() {
                let initial = format!(
                    "---\nschema: hyperscaler.doc.v1\ntitle: {}\nstatus: draft\ncanonical_authority: false\nowner: \"@team/core\"\nlast_verified_at: \"2026-08-19\"\n---\n\n# {}\n\nAuto-generated documentation stub by Anvil DocGuard.\n",
                    file, file
                );
                let _ = tokio::fs::write(&path, initial).await;
                updated.push(file.clone());
            }
        }
        Ok(updated)
    }
}

fn extract_json_block(text: &str) -> Option<String> {
    let re = Regex::new(r"```json\s*([\s\S]*?)\s*```").ok()?;
    if let Some(caps) = re.captures(text) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }
    if text.trim().starts_with('{') && text.trim().ends_with('}') {
        return Some(text.trim().to_string());
    }
    None
}

#[cfg(test)]
mod insufficient_docs_tests {
    //! Issue #29: absent or failed evidence is never a pass.
    //!
    //! These drive `apply_doc_parity_evaluation` directly. The public entry
    //! point, `ensure_documentation_parity`, reaches this behaviour only after
    //! spawning the `agy` doc-parity probe, and a test must never spawn a model.

    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn diff_ctx(repo: &str) -> PrDiffContext {
        PrDiffContext {
            repo: repo.to_string(),
            pr_number: 4242,
            base_branch: "main".to_string(),
            base_sha: "base-sha".to_string(),
            head_sha: "head-sha".to_string(),
            is_incremental: false,
            previous_head_sha: None,
            diff_content: "diff --git a/src/lib.rs b/src/lib.rs\n+pub fn newly_public() {}\n"
                .to_string(),
            changed_files: vec!["src/lib.rs".to_string()],
            repo_working_dir: PathBuf::from("."),
        }
    }

    fn insufficient(files: &[&str]) -> DocParityEvaluation {
        DocParityEvaluation {
            is_doc_sufficient: false,
            missing_doc_summary: Some(
                "newly_public is a new public API with no reference page".to_string(),
            ),
            doc_files_to_update: files.iter().map(|f| (*f).to_string()).collect(),
            suggested_adr_title: None,
        }
    }

    #[tokio::test]
    async fn a_diff_the_probe_judged_under_documented_does_not_yield_a_sufficient_report() {
        let dir = tempdir().unwrap();
        let guard = DocGuard::new("low".to_string());

        let report = guard
            .apply_doc_parity_evaluation(
                "oyatie/anvil",
                dir.path(),
                &diff_ctx("oyatie/anvil"),
                "feat: add a public API",
                "no docs",
                &insufficient(&["docs/reference/newly-public.md"]),
            )
            .await;

        assert!(
            !report.is_sufficient,
            "the probe judged the diff under-documented; the report must not claim \
             sufficiency. summary was: {}",
            report.summary
        );
        // The complement of the failed-write case below. A judgement WAS
        // obtained and the tempdir is writable, so this is a real adverse
        // finding, not absent evidence. Collapsing it into Errored contradicts
        // `DocGuardReport::errored`'s documented contract and would let an
        // implementation that writes nothing, ever, satisfy the honesty tests.
        assert!(
            report.errored.is_none(),
            "a judgement was obtained and the write was possible, so this is a \
             finding and not absent evidence: {:?}",
            report.errored
        );
        assert!(
            dir.path().join("docs/reference/newly-public.md").exists(),
            "the file the probe named does not exist and the directory is \
             writable, so it must actually be written"
        );
        assert!(
            report
                .files_created_or_updated
                .contains(&"docs/reference/newly-public.md".to_string()),
            "the file that was written must be reported as created/updated: {:?}",
            report.files_created_or_updated
        );
    }

    #[tokio::test]
    async fn naming_an_existing_file_that_is_never_amended_cannot_yield_a_pass() {
        let dir = tempdir().unwrap();
        let readme = dir.path().join("README.md");
        let before = "# Watched\n\nNothing here mentions newly_public.\n";
        std::fs::write(&readme, before).unwrap();

        let guard = DocGuard::new("low".to_string());
        let report = guard
            .apply_doc_parity_evaluation(
                "oyatie/anvil",
                dir.path(),
                &diff_ctx("oyatie/anvil"),
                "feat: add a public API",
                "no docs",
                // One file that does not exist and one that does. Creating a
                // stub for the first must not license passing on the second.
                &insufficient(&["docs/reference/newly-public.md", "README.md"]),
            )
            .await;

        let after = std::fs::read_to_string(&readme).unwrap();

        // Unconditional: whatever the guard decides to do about an existing
        // file it was told to update, clobbering the contributor's prose is not
        // one of the options. "Amending" by overwriting with a generated stub
        // is the same vandalism class as issues #27 and #28.
        assert!(
            after.contains("# Watched"),
            "the existing README's heading must survive: {after:?}"
        );
        assert!(
            after.contains("Nothing here mentions newly_public."),
            "the existing README's prose must survive: {after:?}"
        );

        // Two legitimate outcomes, and both have to be honest.
        if after == before {
            assert!(
                !report.is_sufficient,
                "README.md was named as needing an update and was never amended; \
                 that is absent evidence, not a pass. summary was: {}",
                report.summary
            );
            assert!(
                !report
                    .files_created_or_updated
                    .contains(&"README.md".to_string()),
                "README.md is byte-identical, so it must not be reported as updated: {:?}",
                report.files_created_or_updated
            );
        } else {
            assert!(
                report
                    .files_created_or_updated
                    .contains(&"README.md".to_string()),
                "README.md was amended, so it must be reported as updated: {:?}",
                report.files_created_or_updated
            );
        }
    }

    #[tokio::test]
    async fn a_documentation_write_that_failed_is_errored_and_never_reported_as_updated() {
        let dir = tempdir().unwrap();
        // `docs` is a regular file, so no file under `docs/reference/` can be
        // created: every write to that path fails.
        std::fs::write(dir.path().join("docs"), "not a directory\n").unwrap();

        let guard = DocGuard::new("low".to_string());
        let report = guard
            .apply_doc_parity_evaluation(
                "oyatie/anvil",
                dir.path(),
                &diff_ctx("oyatie/anvil"),
                "feat: add a public API",
                "no docs",
                &insufficient(&["docs/reference/newly-public.md"]),
            )
            .await;

        assert!(
            !dir.path().join("docs/reference/newly-public.md").exists(),
            "precondition: the write cannot have succeeded"
        );
        assert!(
            report.errored.is_some(),
            "a write that failed is absent evidence and must be Errored. summary was: {}",
            report.summary
        );
        assert!(
            !report.is_sufficient,
            "a failed write must not pass the gate. summary was: {}",
            report.summary
        );
        assert!(
            report.files_created_or_updated.is_empty(),
            "nothing was written, so nothing may be reported as AutoUpdated: {:?}",
            report.files_created_or_updated
        );
    }
}
