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

    /// Constructs a guard whose doc-parity probe *outcome* is supplied directly
    /// instead of being obtained by spawning the `agy` probe.
    ///
    /// SCAFFOLDING (`tdd/docguard-oracle-repair`): signature only, body left to
    /// the implementer.
    ///
    /// Both of the behaviours the specification asks for on the far side of the
    /// probe — issue #29's "a diff the probe judged insufficient does not yield
    /// a sufficient report", and issue #27's "the gate's summary must state the
    /// sync did not apply" — are only reachable through
    /// `ensure_documentation_parity` after a model has run, and no test may
    /// spawn a model.
    ///
    /// # Contract
    ///
    /// This signature is pinned by `tests/docguard_oracle_repair_test.rs`. It is
    /// not a suggestion the implementer may substitute a different shape for;
    /// changing it edits the specification and requires a fresh test review.
    ///
    /// `outcome` is what `evaluate_doc_parity` would have *returned*, not merely
    /// what the probe would have judged:
    ///
    /// * `Ok(evaluation)` — the probe produced a judgement.
    /// * `Err(reason)` — the probe produced **no** judgement: spawn failure,
    ///   non-zero exit, timeout, unparseable JSON, or watchdog supervision
    ///   failure. All five are the same case to this gate and all five must
    ///   reach `DocGuardReport::errored`.
    ///
    /// The `Err` arm is not a convenience. It is the arm whose historical
    /// collapse into `is_doc_sufficient: true` made gate 1 unfailable, and a
    /// seam that could only express a *successful* judgement would leave that
    /// arm reachable only from production. `Err(reason)` must be delivered to
    /// the same code path a real probe failure takes — an `Err` out of
    /// `evaluate_doc_parity` — so that the failure handling the suite exercises
    /// is the failure handling production runs.
    ///
    /// The outcome is a **stored value, not a slot that empties**. Every call to
    /// `ensure_documentation_parity` on a guard built this way observes it, and
    /// there is no "override exhausted" state in which spawning `agy` becomes
    /// legal again. A `take()`-able slot that falls through to the real spawn
    /// once drained passes every case in the suite as written today and puts
    /// `agy --dangerously-skip-permissions`, on a 120-second budget, one retry
    /// loop or one shared guard away from running inside `cargo test`.
    /// `the_probe_outcome_is_not_consumed_by_the_first_run_of_the_gate` pins
    /// this: the gate is run twice on one guard and the two reports must agree.
    ///
    /// The stored outcome must be consulted **inside `evaluate_doc_parity`**,
    /// at the point where the `agy` probe's judgement is produced and returned,
    /// so that an overridden run and a production run traverse byte-identical
    /// code from the judgement onward. An override consulted earlier — one that
    /// returns from `ensure_documentation_parity` before the corpus sync, or
    /// jumps straight to a report-composing helper — would let the whole suite
    /// go green over an entry point whose real path still passes every
    /// under-documented diff. That is the defect class this branch exists to
    /// remove, so it must not be reintroduced by the seam that tests it.
    ///
    /// The body stays `todo!()` until then: a seam that stores the outcome
    /// without anything consulting it would make a live `agy` spawn reachable
    /// from the suite, and panicking at construction makes that impossible.
    #[allow(unused_variables)]
    pub fn with_probe_override(
        agy_effort: String,
        outcome: Result<DocParityEvaluation, String>,
    ) -> Self {
        todo!("supply the doc-parity probe outcome without spawning the agy probe")
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
