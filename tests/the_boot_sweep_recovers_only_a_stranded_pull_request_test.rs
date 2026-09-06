//! The boot recovery sweep dispatched every uncertified pull request into a
//! guard whose first condition is the reason the stranded ones are stranded.
//!
//! `execute_pr_review` stamps `last_reviewed_head_sha` immediately after
//! posting the review and before the gate corpus, the attestation receipt, the
//! scorecard and the enlist decision -- most of the run's wall clock. Every
//! in-process abort releases that stamp; a killed process cannot, and the
//! daemon exits through `std::process::exit(0)` with every review pipeline in a
//! detached task. So a restart leaves pull requests carrying a stamp with
//! nothing behind it, and the guard reads the stamp as "already handled".
//!
//! The fix cannot be `force: true` on the recovery dispatch. The sweep selects
//! every pull request with no certification recorded at its head, and that set
//! also holds every run the pipeline FINISHED and deliberately halted -- gates
//! failed, review asked for changes past the fixer's bound, cross-repository.
//! Forcing those re-runs a model turn and posts a second review, on every boot,
//! forever. A recovery that recovers everything is as wrong as one that
//! recovers nothing.
//!
//! So the two are separated by a fact neither previously recorded: whether a
//! run reached the END of the pipeline. This file measures both directions --
//! the stranded pull request is admitted, the finished one is not -- across the
//! composition the boot path actually uses: `recovery::needs_certification`
//! selects, then `webhook::pipelines::admit::admit` decides.

use anvil::recovery::needs_certification;
use anvil::source_scan::{code_only, paths::module_source};
use anvil::state::{PrState, StateManager};
use anvil::webhook::pipelines::admit::{Admission, admit};
use std::path::{Path, PathBuf};
use tempfile::tempdir;

const HEAD: &str = "c0ffee1234567890";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// A pull request the review pipeline stamped and never finished.
fn stranded() -> PrState {
    PrState {
        last_reviewed_head_sha: HEAD.to_string(),
        ..PrState::default()
    }
}

/// A pull request the pipeline ran to the end and left uncertified on purpose.
fn finished_and_halted() -> PrState {
    PrState {
        last_reviewed_head_sha: HEAD.to_string(),
        last_completed_head_sha: Some(HEAD.to_string()),
        ..PrState::default()
    }
}

/// What the boot path does with one pull request, end to end.
///
/// The sweep's selection and the pipeline's admission are two decisions taken
/// in two modules, and the defect lived in neither alone: the sweep selected
/// correctly and the guard refused correctly-by-its-own-lights. Only the
/// composition is wrong, so only the composition is worth asserting.
fn boot_dispatch(prior: Option<&PrState>) -> Option<Admission> {
    if !needs_certification(prior, HEAD) {
        return None;
    }
    Some(admit(false, prior, HEAD))
}

#[test]
fn the_sweep_recovers_a_pull_request_a_restart_stranded() {
    let pr = stranded();
    assert_eq!(
        boot_dispatch(Some(&pr)),
        Some(Admission::Recovering),
        "the pull request the recovery sweep exists for is the one it refuses. \
         It carries a reviewed-SHA stamp because a run was killed after posting \
         the review and before the corpus, so the guard reads it as handled and \
         returns -- while the operator log says a review and a full \
         certification were dispatched. Nothing else picks it up: the webhook \
         path takes the same guard, and no new commit is coming."
    );
}

#[test]
fn the_sweep_leaves_a_pull_request_the_pipeline_finished_alone() {
    let pr = finished_and_halted();
    assert_eq!(
        boot_dispatch(Some(&pr)),
        Some(Admission::Skip),
        "a run that reached the end of the pipeline and halted there is not \
         stranded, and re-reviewing it costs a model turn and posts a second \
         review for a head that already has one -- on every boot, for as long \
         as the pull request stays open."
    );
}

/// Recovery must be spent by the recovery. Otherwise the fix is the second
/// defect: every boot re-reviews the same pull request forever.
#[tokio::test]
async fn a_recovered_pull_request_is_not_recovered_twice() {
    let tmp = tempdir().expect("tempdir");
    let sm = StateManager::load(tmp.path()).await.expect("state manager");

    sm.update_pr_state("oyatie/anvil", 5, HEAD.to_string(), Some("APPROVE".into()))
        .await
        .expect("the early stamp");
    let stamped = sm.get_pr_state("oyatie/anvil", 5).await;
    assert_eq!(boot_dispatch(stamped.as_ref()), Some(Admission::Recovering));

    sm.record_pipeline_completion("oyatie/anvil", 5, HEAD)
        .await
        .expect("the completion write at the tail of the pipeline");
    let finished = sm.get_pr_state("oyatie/anvil", 5).await;
    assert_eq!(
        boot_dispatch(finished.as_ref()),
        Some(Admission::Skip),
        "the recovery run finished and recorded it, so this head is answered. \
         Recovering it again is an unbounded re-review loop."
    );
}

/// The marker is only useful if a restart can read it -- the whole failure it
/// answers is a restart.
#[tokio::test]
async fn the_completion_marker_survives_the_restart_it_is_read_across() {
    let tmp = tempdir().expect("tempdir");
    {
        let sm = StateManager::load(tmp.path()).await.expect("state manager");
        sm.update_pr_state("oyatie/anvil", 9, HEAD.to_string(), None)
            .await
            .expect("stamped");
        sm.record_pipeline_completion("oyatie/anvil", 9, HEAD)
            .await
            .expect("completed");
    }
    let reloaded = StateManager::load(tmp.path()).await.expect("reloaded");
    let after = reloaded.get_pr_state("oyatie/anvil", 9).await;
    assert_eq!(
        boot_dispatch(after.as_ref()),
        Some(Admission::Skip),
        "the completion did not survive the restart, so every boot reads a \
         finished pull request as stranded and reviews it again"
    );
}

/// The cases either side of the pair above, so a predicate that answers
/// `Recovering` or `Skip` unconditionally cannot pass this file.
#[test]
fn the_rest_of_the_boot_set_is_unchanged() {
    assert_eq!(
        boot_dispatch(None),
        Some(Admission::Unreviewed),
        "a pull request with no state has never been reviewed"
    );

    let mut certified = finished_and_halted();
    certified.last_certified_head_sha = Some(HEAD.to_string());
    certified.is_enlisted_in_merge_queue = true;
    assert_eq!(
        boot_dispatch(Some(&certified)),
        None,
        "a certified and enlisted head is not in the sweep's set at all"
    );

    let mut rolled_back = stranded();
    rolled_back.last_reviewed_head_sha.clear();
    assert_eq!(
        boot_dispatch(Some(&rolled_back)),
        Some(Admission::Unreviewed),
        "an in-process abort released the stamp; this is an ordinary review, \
         not a recovery, and a cleared stamp must not read as one"
    );

    let mut older_head = finished_and_halted();
    older_head.last_reviewed_head_sha = "0000000000000000".to_string();
    older_head.last_completed_head_sha = Some("0000000000000000".to_string());
    assert_eq!(
        boot_dispatch(Some(&older_head)),
        Some(Admission::Unreviewed),
        "a new commit landed; the completion recorded for the previous head \
         must not vouch for this one"
    );

    assert_eq!(
        admit(true, Some(&finished_and_halted()), HEAD),
        Admission::Forced,
        "the manual door still reviews a head that was already answered"
    );
}

/// The predicates above are decisions nobody takes unless the pipeline calls
/// them. Read against the module rather than a filename, so a split of
/// `pipelines/` does not turn this into an empty walk.
#[test]
fn the_review_pipeline_takes_its_skip_decision_from_admit() {
    let src = module_source("src/webhook/pipelines", &repo_root());
    let code = code_only(&src);

    assert!(
        code.contains("admit::admit(force, state_entry.as_ref(), head_sha)"),
        "`execute_pr_review` no longer asks `admit` whether to skip. Whatever \
         guard replaced it is the guard that stranded the pull requests this \
         file is about."
    );
    assert!(
        !code.contains("last_sha == head_sha"),
        "the old stamp-equality guard is back in the pipeline. It refuses \
         every pull request a restart stranded, because the stamp is what \
         strands them."
    );
}

/// And the completion the predicate reads is actually written, last, on the
/// path that finishes.
#[test]
fn the_review_pipeline_records_that_it_reached_the_end() {
    let src = module_source("src/webhook/pipelines", &repo_root());
    let code = code_only(&src);

    let cert = code
        .find("record::certification(")
        .expect("the pipeline still records what it certified");
    let done = code.find("record::completion(").expect(
        "nothing records that the pipeline reached the end, so every \
             finished-and-halted pull request reads as stranded and is \
             re-reviewed on every boot",
    );
    assert!(
        cert < done,
        "completion is recorded before certification. A death between the two \
         then leaves a head that reads as finished with no record of the \
         enlistment, which is the certification hole `record.rs` documents."
    );
    assert!(
        code.contains("record_pipeline_completion"),
        "`record::completion` reaches no durable writer, so the marker does \
         not survive the restart it exists to be read across"
    );
}
