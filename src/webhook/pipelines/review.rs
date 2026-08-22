use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::webhook::AppState;

#[allow(clippy::too_many_arguments)]
pub async fn execute_pr_review(
    state: &AppState,
    repo: &str,
    pr_number: u64,
    title: &str,
    body: &str,
    base_branch: &str,
    base_sha: &str,
    head_sha: &str,
    force: bool,
) -> Result<()> {
    info!(
        "Executing AI code review and {}-gate certification for {}#{}...",
        crate::pre_merge_guard::report::TOTAL_GATES,
        repo,
        pr_number
    );

    // Acquire exclusive per-PR lock to prevent TOCTOU race conditions from rapid webhook bursts
    let pr_lock = state.state_mgr.acquire_pr_lock(repo, pr_number).await;
    let _guard = pr_lock.lock().await;

    let pipeline_start = std::time::Instant::now();

    let state_entry = state.state_mgr.get_pr_state(repo, pr_number).await;
    let prev_sha = state_entry
        .as_ref()
        .map(|s| s.last_reviewed_head_sha.as_str());

    if !force
        && let Some(last_sha) = prev_sha
        && last_sha == head_sha
    {
        info!(
            "PR {}#{} HEAD {} was already reviewed. Skipping.",
            repo, pr_number, head_sha
        );
        return Ok(());
    }

    let repo_dir = state
        .git_mgr
        .ensure_repo_cloned(repo)
        .await
        .context("Failed to ensure repo cloned")?;

    let diff_ctx = state
        .git_mgr
        .prepare_pr_diff(repo, pr_number, base_branch, base_sha, head_sha, prev_sha)
        .await
        .context("Failed to prepare PR diff context")?;

    if diff_ctx.diff_content.trim().is_empty() {
        info!("No diff found for {}#{}, skipping review.", repo, pr_number);
        return Ok(());
    }

    // 1. Canonical 16-Lens Adversarial Code Review via AI Subscription Driver
    let review_resp = state.reviewer.review_pr(&diff_ctx, title, body).await?;

    info!(
        "Submitting AI Code Review to GitHub for {}#{}...",
        repo, pr_number
    );
    state
        .github_client
        .submit_pr_review(repo, pr_number, head_sha, &review_resp)
        .await?;

    state
        .state_mgr
        .update_pr_state(
            repo,
            pr_number,
            head_sha.to_string(),
            Some(review_resp.verdict.clone()),
        )
        .await?;

    // The repository's own verification gate, run rather than assumed, and run
    // against `head_sha` rather than against whatever the shared clone happens
    // to be on. This used to be a literal `Some(true)` for a suite nothing in
    // this pipeline ran, which the corpus turned into `test_suite_status:
    // Passed` and the approving review published as a measured pass.
    let test_suite_passed =
        super::certify::local_verification_gate(&state.git_mgr, repo, pr_number, head_sha).await;

    // 2..69. The gate corpus, run for this pull request.
    let cert_report = match super::certify::certify_pull_request(
        state,
        repo,
        pr_number,
        title,
        body,
        head_sha,
        &repo_dir,
        &diff_ctx,
        &review_resp.verdict,
        test_suite_passed,
    )
    .await
    {
        Ok(report) => report,
        Err(e) => {
            // Roll back the reviewed-SHA stamp so this PR is retried rather
            // than stranded: the stamp is set above, and the early-exit guard
            // would otherwise skip every later webhook for this SHA. The stamp
            // belongs to this pipeline, so the rollback does too — the corpus
            // is shared with the enlistment paths, and an enlist attempt must
            // not be able to un-stamp a pull request.
            state.state_mgr.clear_reviewed_sha(repo, pr_number).await;
            return Err(e);
        }
    };

    // Re-stamp the provenance receipt with the verdict that was actually
    // computed. The first stamp above records only that the receipt mechanism
    // works; it deliberately carries PENDING_CERTIFICATION because at that
    // point no gate has run. Invariant I2: never report a value you did not
    // measure.
    let final_verdict = if cert_report.admission_refusal().is_ok() {
        "CERTIFIED_READY"
    } else if !cert_report.unmeasured_gates.is_empty() {
        "BLOCKED_UNMEASURED"
    } else {
        "BLOCKED_NOT_CERTIFIED"
    };
    // Positional placeholders named nothing: the index came from a filtered
    // list, so it did not even map back to a field. A receipt that cannot say
    // which gates passed is not evidence.
    let verified_gates: Vec<String> = cert_report
        .named_statuses()
        .into_iter()
        .filter(|(_, s)| matches!(s, crate::pre_merge_guard::GateStatus::Passed))
        .map(|(name, _)| name.to_string())
        .collect();
    if let Err(e) = state
        .attestation_guard
        .stamp_lane_receipt(
            &repo_dir,
            repo,
            pr_number,
            head_sha,
            final_verdict,
            verified_gates,
        )
        .await
    {
        warn!(
            "Could not finalize attestation receipt for {}#{}: {}",
            repo, pr_number, e
        );
    }

    // Post or amend the scorecard in place, keyed on its marker (Zero Clutter).
    state
        .github_client
        .upsert_pr_comment(
            repo,
            pr_number,
            "<!-- ANVIL_SCORECARD_RECEIPT -->",
            &scorecard_comment(&cert_report),
        )
        .await?;

    info!(
        "Pre-Merge, GitOps, CI Velocity & Security Certification completed for {}#{}. Ready: {}",
        repo, pr_number, cert_report.is_certified_ready
    );

    let duration_secs = pipeline_start.elapsed().as_secs();
    let estimated_tokens = ((diff_ctx.diff_content.len() + 2000) as f64 / 3.8).ceil() as usize;
    let _ = state
        .self_governor
        .quota
        .record_model_spend("gemini-3.7-flash", estimated_tokens);

    // Real counts, computed from the gate statuses. These were hardcoded as
    // (70, 0) / (69, 1), so every failing PR was recorded as exactly one failed
    // gate no matter how many actually failed -- which is why the accumulated
    // telemetry showed ~95% of PRs "stuck at 69/70". That was the constant, not
    // a measurement (invariant I2).
    let (gates_passed, gates_failed) = cert_report.gate_counts();

    // Record WHICH gates failed, not just how many. `record_gate_failure` and
    // GateFailureRecord already existed but had no callers, so the gate_failures
    // sink in telemetry_journal.json has been empty for its whole life -- leaving
    // no failure taxonomy to act on.
    for (gate_name, status) in cert_report.named_statuses() {
        let reason = match status {
            crate::pre_merge_guard::report::GateStatus::Failed(r) => Some(r.clone()),
            crate::pre_merge_guard::report::GateStatus::Errored(r) => {
                Some(format!("ERRORED: {}", r))
            }
            crate::pre_merge_guard::report::GateStatus::NotMeasured { reason, .. } => {
                Some(format!("NOT_MEASURED: {}", reason))
            }
            _ => None,
        };
        if let Some(failure_reason) = reason {
            state
                .telemetry_store
                .record_gate_failure(crate::telemetry_store::GateFailureRecord {
                    repo: repo.to_string(),
                    pr_number,
                    gate_name: gate_name.to_string(),
                    failure_reason,
                    timestamp: chrono::Utc::now(),
                })
                .await;
        }
    }

    state
        .telemetry_store
        .record_pr_event(crate::telemetry_store::FleetPrRecord {
            repo: repo.to_string(),
            pr_number,
            title: title.to_string(),
            author: "git-author".to_string(),
            head_sha: head_sha.to_string(),
            review_verdict: review_resp.verdict.clone(),
            gates_passed,
            gates_failed,
            duration_seconds: duration_secs,
            is_certified: cert_report.is_certified_ready,
            recorded_at: chrono::Utc::now(),
        })
        .await;

    state
        .broadcaster
        .broadcast_event(crate::webhook::sse::FleetEventMessage {
            event_type: "pr_review_certified".to_string(),
            repo: repo.to_string(),
            entity_id: format!("PR #{}", pr_number),
            title: format!(
                "{} ({}/{} gates)",
                title,
                gates_passed,
                crate::pre_merge_guard::report::TOTAL_GATES
            ),
            status: if cert_report.is_certified_ready {
                "CERTIFIED".to_string()
            } else {
                "BLOCKED".to_string()
            },
            timestamp_utc: chrono::Utc::now().to_rfc3339(),
            payload_json: None,
        });

    // The admission decision is taken once, by the entry point, on the report
    // this run produced. There is deliberately no pre-check here: a second
    // predicate over the same fields — `is_admissible()`, which cannot see
    // `Errored` and cannot see provenance — was a weaker question asked
    // immediately before the stricter one, and the two agreed only by accident.
    if !cert_report.unmeasured_gates.is_empty() {
        warn!(
            "PR {}#{} withheld from merge queue: {} gate(s) produced no measurement: {}",
            repo,
            pr_number,
            cert_report.unmeasured_gates.len(),
            cert_report.unmeasured_gates.join(", ")
        );
    }
    let enlistment = state
        .merge_enlister
        .enlist_into_merge_queue(repo, pr_number, Some(&cert_report))
        .await;
    if let Err(e) = &enlistment {
        warn!("Automatic merge queue enlistment notice: {}", e);
    }

    // Record which head this run certified, and whether it went into the queue.
    //
    // `StateManager::record_certification` and the two fields it writes have
    // existed all along with no writer anywhere in `src/`, while two live
    // readers depend on them: the `pull_request` webhook's anti-loop filter
    // (`webhook_handlers.rs`) drops a webhook only for a head already certified
    // and queued, and the outage-recovery sweep
    // (`recovery/reconciliation_sweep.rs`) decides a pull request needs
    // certification when the recorded head is not its current one. With nothing
    // writing them, the filter never fired and the sweep re-certified every open
    // pull request on every pass. That is the same defect class as the rest of
    // this change from the other side: a decision taken on a field that no
    // measurement ever reaches.
    //
    // This is the one writer, and it is the review pipeline rather than the
    // enlist doors, because this is the path that has both run the corpus and
    // seen what the merge queue did with it. `CertifiedSubject` on the report
    // answers "is this report about this commit" for one enlistment; this
    // answers "which head has been certified for this pull request" across
    // process restarts. They are not two spellings of one fact, and only this
    // one is durable.
    //
    // Written only for a head the corpus actually certified, on the same
    // question `enlist_into_merge_queue` asks. Stamped for a refused head it
    // would tell the recovery sweep that a blocked pull request needs no
    // further certification -- recording the field on a run that refused would
    // be the field asserting something the run did not find.
    if cert_report.admission_refusal().is_ok()
        && let Err(e) = state
            .state_mgr
            .record_certification(repo, pr_number, head_sha, enlistment.is_ok())
            .await
    {
        warn!(
            "Could not persist the certification record for {}#{}: {}",
            repo, pr_number, e
        );
    }

    Ok(())
}

/// The body published under the scorecard marker.
///
/// Delegates to `crate::publish::scorecard::render`: findings only, passing
/// gates counted rather than enumerated, marker first and signature last. The
/// 68-row matrix `evaluator.rs` still stores in `summary_markdown` is no longer
/// what gets posted -- sixty-odd `PASSED` rows buried the two or three that
/// needed action.
///
/// Kept as a named function rather than an inline call so the upsert call site
/// names the renderer at the argument position, which is what the wiring test
/// asserts against (I22: enforced by mechanism, not by convention).
pub fn scorecard_comment(
    report: &crate::pre_merge_guard::report::PreMergeCertificationReport,
) -> String {
    crate::publish::scorecard::render(report)
}
