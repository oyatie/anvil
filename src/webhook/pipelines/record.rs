//! What a finished review run writes down about itself.
//!
//! Split from `review.rs`, which is over ADR-0719 D-35's budget: the run and
//! the record of the run are two things, and only the second has readers
//! outside this pipeline.

use tracing::warn;

use crate::pre_merge_guard::report::PreMergeCertificationReport;
use crate::webhook::AppState;

pub async fn certification(
    state: &AppState,
    repo: &str,
    pr_number: u64,
    head_sha: &str,
    cert_report: &PreMergeCertificationReport,
    enlisted: bool,
) {
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
    // Written only for a head the corpus certified AND the merge queue took.
    // Stamped for a refused head it would tell the recovery sweep that a
    // blocked pull request needs no further certification -- recording the
    // field on a run that refused would be the field asserting something the
    // run did not find.
    //
    // `enlistment.is_ok()` is part of the condition and not only the value.
    // `needs_cert` in `recovery/reconciliation_sweep.rs` is
    // `last_certified_head_sha != head_sha`, and `cli/server.rs` dispatches
    // `execute_pr_review` for exactly the pull requests the sweep marks
    // uncertified. Written for a certified head whose `gh pr merge` failed -- a
    // rate limit, a `--match-head-commit` race, the queue temporarily disabled
    // -- the field would remove that pull request from the outage-recovery
    // dispatch set on every subsequent daemon start, permanently, at that head:
    // the anti-loop filter in `webhook_handlers.rs` also requires
    // `is_enlisted_in_merge_queue`, so nothing else would pick it back up once
    // the contributor stopped pushing. Before this writer existed the sweep
    // retried on every pass; a new writer must not silently disable the retry
    // path.
    if cert_report.admission_refusal().is_ok()
        && enlisted
        && let Err(e) = state
            .state_mgr
            .record_certification(repo, pr_number, head_sha, enlisted)
            .await
    {
        warn!(
            "Could not persist the certification record for {}#{}: {}",
            repo, pr_number, e
        );
    }
}

/// Record that the pipeline reached the end for this head.
///
/// Unconditional where [`certification`] is conditional, and written after it:
/// this says only that nothing is still owed for `head_sha`, which is true of a
/// halted run and a certified one alike. `PrState::is_stranded_at` reads its
/// absence, so a head that reaches here is never recovered again, and a head
/// whose process died before here always is.
///
/// The two writes are separate calls in `execute_pr_review` rather than one,
/// because they answer different questions and a later reader of either must
/// not have to know the other's condition. Ordered certification-first so that
/// a death between them leaves a pull request that is enlisted and reads as
/// uncertified-but-unfinished, which the next sweep resolves; the reverse order
/// leaves one that reads as finished with no record of the enlistment.
pub async fn completion(state: &AppState, repo: &str, pr_number: u64, head_sha: &str) {
    if let Err(e) = state
        .state_mgr
        .record_pipeline_completion(repo, pr_number, head_sha)
        .await
    {
        warn!(
            "Could not persist the pipeline-completion record for {}#{}: {}. This head will be \
             re-reviewed as stranded on the next dispatch.",
            repo, pr_number, e
        );
    }
}
