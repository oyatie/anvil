use anyhow::{Context, Result};
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

/// Where `evaluate_doc_parity` gets its judgement from.
///
/// # Contract
///
/// This shape is pinned by `tests/docguard_oracle_repair_test.rs` and
/// `tests/docguard_oracle_repair_probe_seam_test.rs`. It has **no empty arm and
/// no drainable arm**, and that is the whole point of it being an enum rather
/// than the `Option`-shaped field the seam would otherwise want:
///
/// * `Option<Result<..>>` (or a `Mutex<Option<..>>` that is `take()`n) has a
///   `None` state, and the only thing an implementation can do in that state is
///   fall through to the real `agy` spawn. A guard built by
///   `with_probe_override` would then spawn
///   `agy --dangerously-skip-permissions`, on a 120-second budget, from inside
///   `cargo test` the moment anything called the gate twice — a retry loop, a
///   shared guard, a second gate invocation.
/// * `Probe` makes that state unrepresentable. A guard is `Live` or it is
///   `Overridden`, permanently, and an `Overridden` guard has an outcome to
///   return on every call. There is no "override exhausted" condition for an
///   implementer to handle, because there is no way to spell one.
///
/// Do not widen this to carry an `Option`, and do not add an arm meaning
/// "used up". `the_probe_outcome_is_not_consumed_by_the_first_run_of_the_gate`
/// pins the behaviour; this type is what stops the mistake from compiling.
pub enum Probe {
    /// Spawn the real `agy` probe at this effort level.
    Live(String),
    /// Return this outcome, on every call, without spawning anything.
    Overridden(Result<DocParityEvaluation, String>),
}

pub struct DocGuard {
    probe: Probe,
}

impl DocGuard {
    pub fn new(agy_effort: String) -> Self {
        Self {
            probe: Probe::Live(agy_effort),
        }
    }

    /// Constructs a guard whose doc-parity probe *outcome* is supplied directly
    /// instead of being obtained by spawning the `agy` probe.
    ///
    /// The behaviours issue #27 and issue #29 ask for are only reachable through
    /// `ensure_documentation_parity` after a model has run, and no test may spawn
    /// a model. `outcome` is what `evaluate_doc_parity` would have *returned*:
    /// `Ok(evaluation)` when the probe produced a judgement, `Err(reason)` when
    /// it produced none (spawn failure, non-zero exit, timeout, unparseable
    /// JSON, supervision failure — all one case to this gate, all reaching
    /// `DocGuardReport::errored`).
    ///
    /// Two requirements, both pinned by `tests/docguard_oracle_repair_test.rs`:
    ///
    /// * The outcome is a stored value, never a slot that empties. See `Probe`.
    /// * It is consulted **inside `evaluate_doc_parity`**, where the probe's
    ///   judgement is produced, so an overridden run and a production run
    ///   traverse byte-identical code from the judgement onward. An override
    ///   consulted earlier — returning before the corpus sync, or jumping to a
    ///   report-composing helper — would let the suite go green over an entry
    ///   point whose real path still passes every under-documented diff.
    ///
    pub fn with_probe_override(outcome: Result<DocParityEvaluation, String>) -> Self {
        Self {
            probe: Probe::Overridden(outcome),
        }
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
        //
        // The sync is scoped to Anvil's own repository. On any other it reports
        // that it did not apply and touches nothing; that skip is a stated fact
        // and is carried into the summary below, because a skip that reads as a
        // clean corpus is a silent pass.
        let (rewritten, skipped_sync) = match corpus_sync::sync_published_counts(
            repo,
            repo_dir,
            crate::pre_merge_guard::report::TOTAL_GATES,
        ) {
            Ok(sync) if !sync.remaining_drift.is_empty() => {
                // A finding, not absent evidence: the sync ran, read the page,
                // and reported what it could not repair. The file list stays
                // EMPTY even though a page was written — the evaluator reads a
                // non-empty list as AutoUpdated, and AutoUpdated certifies.
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
            Ok(sync) => (sync.rewritten, sync.not_applicable),
            Err(e) => {
                return Ok(DocGuardReport {
                    errored: Some(e.to_string()),
                    is_sufficient: false,
                    files_created_or_updated: Vec::new(),
                    summary: format!("Could not make published docs honest: {e}"),
                });
            }
        };
        let sync_skip_note = skipped_sync_note(skipped_sync.as_deref());

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
                summary: format!("{summary}{sync_skip_note}"),
            });
        }

        info!(
            "Missing documentation identified for {}#{}: {:?}. Auto-generating documentation...",
            repo, diff_ctx.pr_number, eval.doc_files_to_update
        );

        // Step 3: Auto-generate missing documentation / ADRs in the workspace.
        //
        // A write that never happened is absent evidence, not an update. It is
        // reported as `errored` with an empty file list, because a file pushed
        // onto `files_created_or_updated` becomes `GateStatus::AutoUpdated` at
        // the evaluator and AutoUpdated certifies.
        let mut updated_files = match self
            .generate_and_write_docs(repo, repo_dir, diff_ctx, pr_title, pr_body, &eval)
            .await
        {
            Ok(files) => files,
            Err(e) => {
                warn!(
                    "DocGuard could not write the documentation it generated for {}#{}: {}",
                    repo, diff_ctx.pr_number, e
                );
                return Ok(DocGuardReport {
                    errored: Some(e.to_string()),
                    is_sufficient: false,
                    files_created_or_updated: Vec::new(),
                    summary: format!(
                        "Documentation updates could not be written: {e}{sync_skip_note}"
                    ),
                });
            }
        };
        updated_files.extend(rewritten);

        // The probe judged this diff under-documented. Generating a stub does
        // not change that judgement: a stub carrying the symbol's name in a
        // heading is evidence of the gap, not its repair. This branch returned
        // a hardcoded `is_sufficient: true`, which is what made gate 1
        // unfailable for every diff the probe actually flagged.
        let mut summary = format!(
            "Documentation parity is insufficient: {}.",
            stated_missing_reason(eval.missing_doc_summary.as_deref())
        );
        if !updated_files.is_empty() {
            summary.push_str(&format!(" Files written: {}.", updated_files.join(", ")));
        }
        summary.push_str(&sync_skip_note);

        Ok(DocGuardReport {
            errored: None,
            is_sufficient: false,
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
2. **Architectural / Doctrine Shifts**: Does this change introduce a new architectural decision, storage pattern, cell boundary, or platform contract that requires an ADR (in `docs/adr/` or `docs/decisions/`)?
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
        // The `Live` arm is what this line always did, verbatim. See
        // `with_probe_override` for why the other two answer from here.
        let agy_effort = match &self.probe {
            Probe::Live(effort) => effort.clone(),
            // The stored outcome is answered HERE, at the point the probe's
            // judgement is produced and returned, so an overridden run and a
            // production run traverse byte-identical code from the judgement
            // onward. It is read, never taken: every call observes it, and
            // there is no state in which spawning `agy` becomes legal again.
            Probe::Overridden(outcome) => {
                return match outcome {
                    Ok(eval) => Ok(eval.clone()),
                    Err(reason) => Err(anyhow::anyhow!("{reason}")),
                };
            }
        };
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
                    Ok(output) => classify_probe_output(
                        output.status,
                        &String::from_utf8_lossy(&output.stdout),
                        &String::from_utf8_lossy(&output.stderr),
                    ),
                    // `run_bounded_for` already distinguishes "failed to run"
                    // from "timed out" in its message, and both stay errors.
                    Err(e) => Err(e),
                }
            },
            // No deterministic local fallback exists for doc parity, so the
            // watchdog path must report the failure rather than manufacture a
            // pass. This arm previously returned is_doc_sufficient: true, which
            // made gate 1 unfailable.
            //
            // `run_with_watchdog` delegates to `run_with_adaptive_watchdog`
            // (watchdog/mod.rs:253), which calls this fallback for both of its
            // own failure modes AND for an operation that returned `Err`, so it
            // is the last thing every unsuccessful probe passes through. It must
            // carry the supervisor's own reason, or a stalled probe, one that
            // exited non-zero and one that printed gibberish all become the same
            // unactionable line on a contributor's scorecard.
            |err| {
                Err(anyhow::anyhow!(
                    "doc parity probe supervision failed: {err}"
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
            if path.exists() {
                // Amending an existing document is not implemented. Overwriting
                // a contributor's prose with a generated stub is the same
                // vandalism class as the corpus sync editing a repository that
                // is not Anvil's, and appending a generic block closes nothing.
                // So the file is left alone AND left out of the reported list:
                // announcing an update that did not happen is exactly the false
                // assurance this gate exists to prevent.
                warn!(
                    "DocGuard was asked to update the existing document {}, which it \
                     cannot yet amend; it is left unchanged and is not reported as \
                     updated",
                    file
                );
                continue;
            }
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("create the parent directory of {file}"))?;
            }
            let initial = format!(
                "---\nschema: hyperscaler.doc.v1\ntitle: {}\nstatus: draft\ncanonical_authority: false\nowner: \"@team/core\"\nlast_verified_at: \"2026-08-19\"\n---\n\n# {}\n\nAuto-generated documentation stub by Anvil DocGuard.\n",
                file, file
            );
            // Not `let _ = ..`: a discarded write error is a file reported as
            // AutoUpdated that was never written.
            tokio::fs::write(&path, initial)
                .await
                .with_context(|| format!("write {file}"))?;
            updated.push(file.clone());
        }
        Ok(updated)
    }
}

/// What the gate appends to its summary when the corpus sync did not apply.
///
/// Empty when it did apply. The announcement is the GATE's own words, and it
/// lives entirely on the skipped side: a summary that carries it on a run where
/// the sync demonstrably applied tells every Anvil pull request something false,
/// on the one repository the sync does own.
fn skipped_sync_note(not_applicable: Option<&str>) -> String {
    match not_applicable {
        Some(reason) => format!(" The published-corpus sync did not apply: {reason}"),
        None => String::new(),
    }
}

/// The reason a blocked pull request is told its documentation is insufficient.
///
/// `missing_doc_summary` is an `Option<String>` deserialised straight out of
/// model JSON, so "the probe said nothing" arrives in three shapes: absent,
/// empty, and whitespace. All three are normalised here, once, to the gate's own
/// words — piping a blank string into the gate's sentence publishes a blocked
/// scorecard row that promises a reason and gives none.
fn stated_missing_reason(missing_doc_summary: Option<&str>) -> String {
    match missing_doc_summary.map(str::trim).filter(|r| !r.is_empty()) {
        Some(reason) => reason.to_string(),
        None => "the probe judged this diff under-documented without stating what is missing"
            .to_string(),
    }
}

/// Decides whether a completed doc-parity probe run produced a judgement.
///
/// SCAFFOLDING (`tdd/docguard-oracle-repair`): a byte-verbatim EXTRACTION of the
/// classification `evaluate_doc_parity`'s probe closure already performed
/// inline, which now calls it. No behaviour moves and no defect is repaired
/// here; the extraction is disclosed at the head of
/// `tests/docguard_oracle_repair_test.rs` together with the cases it makes
/// green — the same treatment, for the same reason, as
/// `pre_merge_guard::evaluator::doc_parity_status`.
///
/// It is public because it is the decision the specification cares about most
/// and the one the suite could not otherwise reach: the boundary between "the
/// probe told us something" and "the probe told us nothing". `Err` here is not a
/// degraded `Ok`. It is the state whose historical collapse into
/// `is_doc_sufficient: true` made gate 1 unfailable, and the comment recording
/// that is still in this file.
///
/// # Contract
///
/// * exit success and a parseable judgement on stdout — `Ok(judgement)`,
///   carrying what the probe actually said, unaltered.
/// * exit success and nothing parseable on stdout — `Err`. A run that printed
///   prose, printed nothing, or was cut off mid-JSON has said nothing about this
///   diff, and "nothing" must never be read as "sufficient".
/// * a non-zero exit — `Err`, carrying the run's own stderr, because that is
///   what tells an operator whether this was a broken invocation or a broken
///   repository.
/// * every `Err` states something, and the categories are tellable apart.
pub fn classify_probe_output(
    status: std::process::ExitStatus,
    stdout: &str,
    stderr: &str,
) -> Result<DocParityEvaluation> {
    if status.success() {
        if let Some(json_str) = extract_json_block(stdout)
            && let Ok(eval) = serde_json::from_str::<DocParityEvaluation>(&json_str)
        {
            return Ok(eval);
        }
        // Ran successfully but produced nothing parseable: we have no
        // judgement, so we must not claim sufficiency.
        anyhow::bail!(
            "doc parity probe returned no parseable evaluation (stdout {} bytes)",
            stdout.len()
        )
    }
    anyhow::bail!(
        "doc parity probe exited with status {}: {}",
        status,
        stderr.trim()
    )
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
