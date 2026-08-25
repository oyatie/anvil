use anyhow::{Context, Result, bail};
use std::sync::Arc;
use tokio::process::Command;
use tracing::{info, warn};

pub mod engine;
pub mod evaluator;

use crate::git_manager::GitManager;
use crate::github::GitHubClient;
use engine::FixEngine;
pub use evaluator::ReviewFeedbackItem;

pub struct Fixer {
    git_mgr: Arc<GitManager>,
    github_client: Arc<GitHubClient>,
    engine: FixEngine,
    agy_effort: String,
}

impl Fixer {
    pub fn new(
        git_mgr: Arc<GitManager>,
        github_client: Arc<GitHubClient>,
        agy_effort: String,
    ) -> Self {
        let engine = FixEngine::new(agy_effort.clone());
        Self {
            git_mgr,
            github_client,
            engine,
            agy_effort,
        }
    }

    /// Resolves feedback items: evaluates signals, applies code fixes, tests, commits, and pushes.
    /// Returns Some(new_commit_sha) if a fix was pushed, or None if no code changes were needed.
    pub async fn resolve_and_fix(
        &self,
        repo: &str,
        pr_number: u64,
        head_branch: &str,
        _head_sha: &str,
        is_cross_repository: bool,
        feedback_items: &[ReviewFeedbackItem],
    ) -> Result<Option<String>> {
        // Refuse fork PRs before doing any work: `HEAD:<head_branch>` would
        // resolve against the BASE repository. See github::fork_guard.
        crate::github::fork_guard::ensure_push_allowed(repo, pr_number, is_cross_repository)?;
        if feedback_items.is_empty() {
            info!(
                "No review feedback items to resolve for {}#{}",
                repo, pr_number
            );
            return Ok(None);
        }

        let repo_dir = self.git_mgr.ensure_repo_cloned(repo).await?;

        // Ensure PR branch is checked out
        let mut fetch_cmd = Command::new("git");
        fetch_cmd.current_dir(&repo_dir).args([
            "fetch",
            "origin",
            &format!("pull/{}/head", pr_number),
            "--force",
        ]);
        let _ = crate::exec::run_bounded(
            fetch_cmd,
            crate::exec::ExecClass::Vcs,
            "git fetch pull head",
        )
        .await;

        let mut checkout_cmd = Command::new("git");
        checkout_cmd.current_dir(&repo_dir).args([
            "checkout",
            "-B",
            &format!("pr-{}", pr_number),
            "FETCH_HEAD",
        ]);
        let _ = crate::exec::run_bounded(
            checkout_cmd,
            crate::exec::ExecClass::Vcs,
            "git checkout PR branch",
        )
        .await;

        info!(
            "Evaluating {} review feedback items for {}#{} on branch {}",
            feedback_items.len(),
            repo,
            pr_number,
            head_branch
        );

        // Step 1: Evaluate signals (Valid Issue vs. False Signal)
        let eval_result =
            evaluator::evaluate_feedback_items(repo, &repo_dir, feedback_items, &self.agy_effort)
                .await?;

        let mut valid_items = Vec::new();
        let mut false_signal_items = Vec::new();

        for eval in eval_result.evaluations {
            if eval.item_index < feedback_items.len() {
                let original = &feedback_items[eval.item_index];
                if eval.is_valid {
                    valid_items.push((original.clone(), eval));
                } else {
                    false_signal_items.push((original.clone(), eval));
                }
            }
        }

        info!(
            "Evaluation complete for {}#{}: {} valid issues, {} false signals",
            repo,
            pr_number,
            valid_items.len(),
            false_signal_items.len()
        );

        // Step 2: Respond to false signals with technical rationale
        for (item, eval) in &false_signal_items {
            if let Some(comment_id) = item.comment_id {
                let reply_text = format!(
                    "🔍 **Feedback Evaluation:**\n\n{}\n\n*Determined as intended behavior / false signal under current architectural invariants.*\n\n---\n*🤖 [Evaluated] by Oyatie Anvil*",
                    eval.rationale
                );
                let _ = self
                    .github_client
                    .reply_to_review_comment(repo, pr_number, comment_id, &reply_text)
                    .await;
            }
        }

        // If no valid items require code changes, we're done
        if valid_items.is_empty() {
            info!(
                "No valid code modifications needed for {}#{}",
                repo, pr_number
            );
            return Ok(None);
        }

        // Step 3: Apply code fixes using Antigravity
        self.engine
            .apply_code_fixes(repo, &repo_dir, &valid_items)
            .await?;

        // Step 4: Run local verification gate (tests/typecheck)
        let test_ok = self.engine.run_test_verification_gate(&repo_dir).await?;
        if !test_ok {
            warn!("Test gate reported failures. Attempting self-correction with Antigravity...");
            self.engine.attempt_self_correction(&repo_dir).await?;
            let retest_ok = self.engine.run_test_verification_gate(&repo_dir).await?;
            if !retest_ok {
                // Previously this only warned "Proceeding with caution" and then
                // committed and pushed anyway, so the verification gate never
                // actually gated anything. AI-authored changes that fail the
                // local suite must not reach the PR branch.
                warn!(
                    "Verification gate still failing for {}#{} after self-correction; \
                     abandoning fix without pushing.",
                    repo, pr_number
                );
                return Ok(None);
            }
        }

        // Step 5: Check git status, commit, and push
        let mut status_cmd = Command::new("git");
        status_cmd
            .current_dir(&repo_dir)
            .args(["status", "--porcelain"]);
        let status_out =
            crate::exec::run_bounded(status_cmd, crate::exec::ExecClass::Quick, "git status")
                .await
                .context("Failed to check git status")?;

        let changes = String::from_utf8_lossy(&status_out.stdout);
        if changes.trim().is_empty() {
            info!(
                "No file changes produced after fix attempt on {}#{}",
                repo, pr_number
            );
            return Ok(None);
        }

        // `repo_dir` is the clone `review.rs` stamps the lane receipt into, so
        // a bare sweep here committed Anvil's own bookkeeping onto the pull
        // request it was fixing.
        let add_cmd = crate::git_manager::stage_excluding_receipts(&repo_dir);
        let _ = crate::exec::run_bounded(add_cmd, crate::exec::ExecClass::Quick, "git add (fixer)")
            .await;

        let commit_msg = format!(
            "fix: address review feedback on PR #{}\n\n\
            Resolved {} review finding(s).\n\n\
            X-Anvil-Action: auto-fix\n\
            X-Anvil-Version: 0.1.0\n\n\
            *🤖 [Fixed] by Oyatie Anvil*",
            pr_number,
            valid_items.len()
        );

        let mut commit_cmd = Command::new("git");
        commit_cmd
            .current_dir(&repo_dir)
            .args(["commit", "-m", &commit_msg]);
        let commit_out =
            crate::exec::run_bounded(commit_cmd, crate::exec::ExecClass::Quick, "git commit")
                .await
                .context("Failed to create fix commit")?;

        if !commit_out.status.success() {
            let err = String::from_utf8_lossy(&commit_out.stderr);
            bail!("git commit failed: {}", err);
        }

        let mut sha_cmd = Command::new("git");
        sha_cmd.current_dir(&repo_dir).args(["rev-parse", "HEAD"]);
        let sha_out =
            crate::exec::run_bounded(sha_cmd, crate::exec::ExecClass::Quick, "git rev-parse HEAD")
                .await?;
        let new_commit_sha = String::from_utf8_lossy(&sha_out.stdout).trim().to_string();

        info!(
            "Created fix commit {} on {}#{}",
            new_commit_sha, repo, pr_number
        );

        info!("Pushing fix to origin branch {}...", head_branch);
        let push_target = format!("HEAD:{}", head_branch);
        let mut push_cmd = Command::new("git");
        push_cmd
            .current_dir(&repo_dir)
            .args(["push", "origin", &push_target]);
        let push_out =
            crate::exec::run_bounded(push_cmd, crate::exec::ExecClass::Vcs, "git push fix commit")
                .await
                .context("Failed to execute git push")?;

        if !push_out.status.success() {
            let err = String::from_utf8_lossy(&push_out.stderr);
            warn!(
                "git push failed to {}: {}. Trying force-with-lease or checking permissions.",
                head_branch, err
            );
        } else {
            info!(
                "Successfully pushed fix commit {} to origin/{}",
                new_commit_sha, head_branch
            );
        }

        let short_sha = if new_commit_sha.len() >= 7 {
            &new_commit_sha[..7]
        } else {
            &new_commit_sha
        };

        for (item, eval) in &valid_items {
            if let Some(comment_id) = item.comment_id {
                let reply_text = format!(
                    "✅ **Addressed in commit [`{}`](https://github.com/{}/commit/{}):**\n\n{}\n\n*Verified against local test suites.*\n\n---\n*🤖 [Fixed] by Oyatie Anvil*",
                    short_sha, repo, new_commit_sha, eval.rationale
                );
                let _ = self
                    .github_client
                    .reply_to_review_comment(repo, pr_number, comment_id, &reply_text)
                    .await;
            }
        }

        Ok(Some(new_commit_sha))
    }
}
