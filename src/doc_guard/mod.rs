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
/// Do not widen this to carry an `Option`, and do not add a third arm meaning
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
    /// once drained puts `agy --dangerously-skip-permissions`, on a 120-second
    /// budget, one retry loop or one shared guard away from running inside
    /// `cargo test`.
    ///
    /// That is a requirement of this contract, and it is also enforced by the
    /// type: `outcome` must be stored as `Probe::Overridden(outcome)` on the
    /// guard's single `probe` field, replacing the `Probe::Live` the guard
    /// would otherwise carry. `Probe` deliberately has no empty arm and no
    /// drainable arm, so "the override has been used up" is a state this code
    /// cannot spell. Do not reintroduce it by adding a second field, an
    /// `Option`, or a `Mutex<Option<..>>` alongside `probe` —
    /// `the_probe_outcome_is_not_consumed_by_the_first_run_of_the_gate` pins
    /// the behaviour (the gate is run twice on one guard and the two reports
    /// must agree), and `Probe` is what stops the mistake from compiling in the
    /// first place.
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
    /// The effort level is discarded rather than stored alongside the outcome:
    /// there is no state in which this guard spawns anything, so there is
    /// nothing for it to be the effort level *of*.
    #[allow(unused_variables)]
    pub fn with_probe_override(
        agy_effort: String,
        outcome: Result<DocParityEvaluation, String>,
    ) -> Self {
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
        // The sync applies only to Anvil's own repository. When it declines, the
        // skip is a stated fact rather than absent evidence — it must neither
        // fail nor rescue the pull request — so it is carried into the summary
        // instead of being allowed to read as a silent pass.
        let sync = match corpus_sync::sync_published_counts(
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
            Ok(sync) => sync,
            Err(e) => {
                return Ok(DocGuardReport {
                    errored: Some(e.to_string()),
                    is_sufficient: false,
                    files_created_or_updated: Vec::new(),
                    summary: format!("Could not make published docs honest: {e}"),
                });
            }
        };
        let rewritten = sync.rewritten;
        // Appended, never interpolated into a fixed phrase: a sync that DID
        // apply must not be described by a sentence that trails off where the
        // reason it did not apply would have gone.
        let skip_statement = match &sync.not_applicable {
            Some(reason) => format!(" Corpus sync did not apply: {reason}"),
            None => String::new(),
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
                    summary: format!(
                        "Documentation parity could not be evaluated: {e}{skip_statement}"
                    ),
                });
            }
        };

        if eval.is_doc_sufficient {
            info!(
                "Documentation parity is satisfied for {}#{}",
                repo, diff_ctx.pr_number
            );
            let base = if rewritten.is_empty() {
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
                summary: format!("{base}{skip_statement}"),
            });
        }

        info!(
            "Missing documentation identified for {}#{}: {:?}. Auto-generating documentation...",
            repo, diff_ctx.pr_number, eval.doc_files_to_update
        );

        // Step 3: Auto-generate missing documentation / ADRs in the workspace.
        //
        // The verdict below is the PROBE's, not a summary of the work this step
        // got done. A stub carrying the symbol's name in a heading is evidence
        // of the documentation gap, not its repair, so writing one cannot turn
        // an adverse judgement into a pass — which is what the discarded
        // `is_sufficient: true` on this path did.
        let generated = generate_and_write_docs(repo_dir, &eval).await;

        let mut files_created_or_updated = generated.written.clone();
        files_created_or_updated.extend(rewritten.iter().cloned());

        let mut statements = vec![match eval
            .missing_doc_summary
            .as_deref()
            .map(str::trim)
            .filter(|reason| !reason.is_empty())
        {
            Some(reason) => format!("Documentation parity is insufficient: {reason}"),
            // A blocked pull request whose scorecard row says nothing is
            // unactionable; the probe declining to explain itself is not a
            // licence for the gate to publish an empty reason.
            None => {
                "Documentation parity is insufficient, and the probe gave no reason.".to_string()
            }
        }];
        if !generated.written.is_empty() {
            statements.push(format!(
                "Documentation stubs generated: {}",
                generated.written.join(", ")
            ));
        }
        if !generated.unamended.is_empty() {
            statements.push(format!(
                "Named but left untouched, because amending an existing document is not \
                 implemented: {}",
                generated.unamended.join(", ")
            ));
        }
        if !rewritten.is_empty() {
            statements.push(format!(
                "Published docs rewritten to TOTAL_GATES={}: {}",
                crate::pre_merge_guard::report::TOTAL_GATES,
                rewritten.join(", ")
            ));
        }
        if !generated.failed.is_empty() {
            statements.push(format!(
                "Documentation could not be written: {}",
                generated.failed.join("; ")
            ));
        }

        Ok(DocGuardReport {
            // A write that never happened is absent evidence, not an update.
            errored: (!generated.failed.is_empty()).then(|| generated.failed.join("; ")),
            is_sufficient: false,
            files_created_or_updated,
            summary: format!("{}{skip_statement}", statements.join(" ")),
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
        // The `Overridden` arm returns the stored outcome from exactly the point
        // the `agy` probe's judgement would have been produced, so an overridden
        // run and a production run traverse byte-identical code from the outcome
        // onward — including the `Err` path, which is the arm whose historical
        // collapse into `is_doc_sufficient: true` made gate 1 unfailable. The
        // outcome is read, never taken: there is no state in which falling
        // through to a real spawn becomes legal. See `with_probe_override`.
        let agy_effort = match &self.probe {
            Probe::Live(effort) => effort.clone(),
            Probe::Overridden(outcome) => return outcome.clone().map_err(anyhow::Error::msg),
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
}

/// What `generate_and_write_docs` actually did, told apart so the report can be
/// honest about each.
#[derive(Debug, Default)]
struct GeneratedDocs {
    /// Files that were named, did not exist, and were written successfully.
    written: Vec<String>,
    /// Files that were named and already existed. DocGuard does not yet amend an
    /// existing document, so these were left exactly as the contributor wrote
    /// them — and are not reported as updated, because they were not.
    unamended: Vec<String>,
    /// Files that were named and could not be written. Reporting one of these as
    /// AutoUpdated claims work that never happened.
    failed: Vec<String>,
}

/// Writes a stub for every named file that does not exist yet.
///
/// Three outcomes, kept separate. The historical version had one: it wrote only
/// when the path did not exist, discarded the write error with `let _ =`, and
/// pushed the file onto the updated list either way — so a file that was skipped
/// and a file whose write failed were both published to the scorecard as
/// AutoUpdated.
async fn generate_and_write_docs(repo_dir: &Path, eval: &DocParityEvaluation) -> GeneratedDocs {
    let mut out = GeneratedDocs::default();
    for file in &eval.doc_files_to_update {
        let path = repo_dir.join(file);
        if path.exists() {
            // Overwriting the contributor's document with a generated stub would
            // be the same vandalism the corpus sync was just scoped to stop.
            warn!("DocGuard was told to update {file}, which already exists; leaving it alone");
            out.unamended.push(file.clone());
            continue;
        }
        if let Some(parent) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            out.failed.push(format!("{file}: {e}"));
            continue;
        }
        let initial = format!(
            "---\nschema: hyperscaler.doc.v1\ntitle: {file}\nstatus: draft\ncanonical_authority: false\nowner: \"@team/core\"\nlast_verified_at: \"2026-08-19\"\n---\n\n# {file}\n\nAuto-generated documentation stub by Anvil DocGuard.\n"
        );
        match tokio::fs::write(&path, initial).await {
            Ok(()) => out.written.push(file.clone()),
            Err(e) => out.failed.push(format!("{file}: {e}")),
        }
    }
    out
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
