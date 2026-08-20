//! Refuses to push to a branch that does not belong to the base repository.
//!
//! `git push origin HEAD:<head_ref_name>` resolves against the BASE repository,
//! not the PR's head repository. For a fork PR the head branch name is chosen
//! entirely by the contributor, so a PR from a fork whose head branch is named
//! `dev`, `main` or `staging` would push Anvil's changes directly into the base
//! repository's branch of that name -- bypassing review, the gate matrix and the
//! merge queue.
//!
//! Anvil could not previously detect this: `gh pr view --json` did not request
//! `isCrossRepository`, so fork PRs were indistinguishable from same-repo PRs.
//!
//! Policy (invariant I4): fork PRs are still reviewed and still receive a
//! scorecard. Only the push is refused.

use anyhow::{Result, bail};
use tracing::warn;

/// Returns an error when the PR head is in a fork, so the caller must not push.
pub fn ensure_push_allowed(repo: &str, pr_number: u64, is_cross_repository: bool) -> Result<()> {
    if is_cross_repository {
        warn!(
            "Refusing to push to {}#{}: the PR head is in a fork, so `HEAD:<branch>` would \
             target the base repository's branch of that name. Review and certification \
             continue; only the push is withheld.",
            repo, pr_number
        );
        bail!(
            "push refused for {}#{}: pull request head is in a forked repository",
            repo,
            pr_number
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_repo_pull_requests_may_be_pushed() {
        assert!(ensure_push_allowed("oyatie/anvil", 9, false).is_ok());
    }

    #[test]
    fn fork_pull_requests_are_refused() {
        // The concrete attack: a fork PR whose head branch is named "dev" would
        // otherwise push into the base repository's dev branch.
        let err = ensure_push_allowed("oyatie/anvil", 1234, true).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("forked repository"), "unexpected: {msg}");
        assert!(msg.contains("1234"));
    }
}
