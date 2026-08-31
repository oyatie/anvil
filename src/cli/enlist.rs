//! The CLI `enlist` door: `anvil enlist --repo <r> --pr <n>`.
//!
//! Extracted from the `Commands::Enlist` arm of `handle_cli` so the door can be
//! driven by a test. `handle_cli` begins with `Cli::parse()`, which reads the
//! real process arguments, so nothing that goes through it can be called from a
//! test binary -- the arm was reachable only by running the shipped executable,
//! and what covered it was a scan of its source text.
//!
//! `pub` rather than `pub(crate)` for the same reason
//! `MergeEnlister::subject_refusal` is: an integration test sees only `pub`
//! items, and a door nothing can call is a door nothing can check.

use anyhow::Result;
use tracing::info;

use crate::webhook::AppState;

/// Certifies `repo#pr` and hands it to the merge queue.
///
/// The certification is run here rather than assumed: this path has not
/// reviewed the pull request, so it has no report unless it produces one.
/// `evidence.as_ref().ok()` is what the door hands over -- a failed run becomes
/// `None`, and `enlist_into_merge_queue` refuses `None`. Converting the failure
/// to `None` is the whole point: absent evidence has to reach the guard as a
/// value, not as an early return the guard never sees.
pub async fn enlist(state: &AppState, repo: &str, pr: u64) -> Result<()> {
    info!(
        "Running on-demand merge queue enlistment for {}#{}",
        repo, pr
    );
    let evidence =
        crate::webhook::pipelines::certify::evidence_for_enlistment(state, repo, pr, None).await;
    // The cause, on the way to the exit code. Collapsed to "no report was
    // obtained" it tells an operator nothing they can act on.
    if let Err(e) = &evidence {
        tracing::warn!("No certification report for {}#{}: {:#}", repo, pr, e);
    }
    state
        .merge_enlister
        .enlist_into_merge_queue(repo, pr, evidence.as_ref().ok())
        .await
}
