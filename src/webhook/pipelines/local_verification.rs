//! The repository's own verification, run at the commit under review.
//!
//! Its own file because `certify` assembles seventy-two gate statuses and is
//! nearly seven hundred lines past the budget; this gate is self-contained and
//! its reasoning is long enough to be worth reading on its own.

use tracing::warn;

/// The repository's own verification gate, run against `head_sha` itself.
///
/// `Some(true)`/`Some(false)` is a gate that ran to completion on this commit;
/// `None` is a gate that could not be attributed to it — the repository offers
/// none, no tree at `head_sha` could be produced, the tree that was produced is
/// not at `head_sha`, or the gate never completed — which the corpus records as
/// `NotMeasured` and which withholds the merge.
///
/// `Some(false)` is reserved for a gate that ran and reported failures, because
/// it is published on the pull request as "Test suite reported failures during
/// verification gate" and counted against the pull request in the approving
/// review. Everything else is a failure to measure, and this function does not
/// convert one into the other in either direction.
///
/// The tree is an ephemeral worktree at `head_sha`, which is the whole of the
/// correctness here — and it is *checked*, with `EphemeralWorktree::verify_at`,
/// rather than assumed from the argument that was passed in:
/// `create_ephemeral_worktree` falls back to `FETCH_HEAD` when the object is
/// not local, and `FETCH_HEAD` in the shared clone is whatever ref was fetched
/// last.
///
/// Run in the shared clone that
/// `ensure_repo_cloned` hands out, this gate builds whatever that clone is
/// currently on: nothing on the review or the certify path ever checks a pull
/// request head out into it (`ensure_repo_cloned` only fetches, and
/// `prepare_pr_diff` only fetches the pull ref), while `execute_pr_fix` runs
/// `git checkout -B pr-<N>` in it. So its outcome was the default branch's, or
/// the last PR the fixer touched — published as this pull request's
/// `test_suite_status`, counted in "N of 72 gates passed", and signed into a
/// formal GitHub APPROVE. A green default branch admitted a pull request that
/// does not compile; a clone left dirty by the fixer accused one nothing
/// measured. `QueueHealer::heal_ejected_pr` already ran this gate correctly, on
/// an ephemeral worktree at the head it had just produced; this is the same
/// mechanism, and the reason its result may be called a measurement of the
/// certified commit.
pub async fn local_verification_gate(
    git_mgr: &crate::git_manager::GitManager,
    repo: &str,
    pr_number: u64,
    head_sha: &str,
) -> Option<bool> {
    let worktree = match git_mgr
        .create_ephemeral_worktree(repo, pr_number, head_sha)
        .await
    {
        Ok(worktree) => worktree,
        Err(e) => {
            // `None`, not `Some(false)`: a tree that could not be produced is
            // not a suite that failed. `NotMeasured` withholds the merge and
            // names itself; a fabricated `Failed` would accuse the pull request
            // of something nothing ran.
            warn!(
                "No tree at {} for {}#{}, so the local verification gate was not measured: {:#}",
                head_sha, repo, pr_number, e
            );
            return None;
        }
    };

    // The tree is a tree; this is what makes it *this pull request's* tree.
    // `create_ephemeral_worktree` falls back to `FETCH_HEAD` in the shared
    // clone when the head object is not local, and `FETCH_HEAD` is whatever was
    // fetched last -- another pull request's head from a concurrent
    // `prepare_pr_diff`, or the base branch. Unchecked, the gate's answer would
    // be a measurement of a different commit published as this one's, counted
    // in the approving review and admitted by `admission_refusal`. `None`
    // again, for the same reason as above: a tree Anvil cannot prove is the
    // certified commit measures nothing about it.
    if let Err(e) = worktree.verify_at(head_sha).await {
        warn!(
            "The local verification gate for {}#{} was not measured: {:#}",
            repo, pr_number, e
        );
        if let Err(e) = worktree.cleanup().await {
            warn!(
                "Verification-gate worktree cleanup failed for {}#{}: {}",
                repo, pr_number, e
            );
        }
        return None;
    }

    let outcome = match crate::queue_healer::QueueHealer::run_local_test_gate(
        &worktree.worktree_path,
    )
    .await
    {
        crate::queue_healer::TestGate::Passed(_) => Some(true),
        crate::queue_healer::TestGate::Failed(_) => Some(false),
        // A gate that never completed is not a suite that failed, and this is
        // the arm that reaches a GitHub comment: `Some(false)` becomes
        // `GateStatus::Failed("Test suite reported failures during verification
        // gate.")` on the scorecard, with a remediation telling the contributor
        // to fix tests that were never run. `cargo` missing from the daemon's
        // PATH, the `ExecClass::Build` deadline expiring on a cold build, and
        // the worktree GC reaping this tree mid-build all arrive here.
        crate::queue_healer::TestGate::Errored(label, cause) => {
            warn!(
                "The local verification gate `{}` for {}#{} did not complete, so it was not \
                 measured: {}",
                label, repo, pr_number, cause
            );
            None
        }
        crate::queue_healer::TestGate::Unavailable => None,
    };

    if let Err(e) = worktree.cleanup().await {
        warn!(
            "Verification-gate worktree cleanup failed for {}#{}: {}",
            repo, pr_number, e
        );
    }
    outcome
}
