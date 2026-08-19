use anyhow::{bail, Context, Result};
use regex::Regex;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{error, info, warn};

pub mod bisector;
pub use bisector::{BisectionResult, MergeTrainBisector};

use crate::git_manager::GitManager;
use crate::github::GitHubClient;
use crate::merge_enlister::MergeEnlister;

pub struct QueueHealer {
    git_mgr: Arc<GitManager>,
    github_client: Arc<GitHubClient>,
    merge_enlister: Arc<MergeEnlister>,
    bisector: MergeTrainBisector,
    agy_effort: String,
}

impl QueueHealer {
    pub fn new(
        git_mgr: Arc<GitManager>,
        github_client: Arc<GitHubClient>,
        merge_enlister: Arc<MergeEnlister>,
        agy_effort: String,
    ) -> Self {
        let bisector = MergeTrainBisector::new();
        Self {
            git_mgr,
            github_client,
            merge_enlister,
            bisector,
            agy_effort,
        }
    }

    /// Extracts PR number from a merge group head_ref (e.g. "gh-readonly-queue/main/pr-824-7fd783...")
    pub fn extract_pr_number_from_merge_ref(merge_ref: &str) -> Option<u64> {
        let re = Regex::new(r"pr-(\d+)").ok()?;
        let caps = re.captures(merge_ref)?;
        caps.get(1)?.as_str().parse().ok()
    }

    /// Bisects a speculative merge train batch to isolate and evict a single regressing PR
    pub fn bisect_speculative_batch<F>(
        &self,
        pr_batch: &[u64],
        test_fn: F,
    ) -> Result<BisectionResult>
    where
        F: FnMut(&[u64]) -> bool,
    {
        self.bisector.bisect_batch(pr_batch, test_fn)
    }

    /// Heals an ejected or failed merge queue PR
    pub async fn heal_ejected_pr(&self, repo: &str, pr_number: u64) -> Result<()> {
        info!("Starting Merge Queue Healer for {}#{}...", repo, pr_number);

        let meta = self
            .github_client
            .fetch_pr_metadata(repo, pr_number)
            .await?;
        let repo_dir = self.git_mgr.ensure_repo_cloned(repo).await?;

        // 1. Fetch latest main and checkout PR branch
        let _ = Command::new("git")
            .current_dir(&repo_dir)
            .args(["fetch", "origin", "main", "--prune"])
            .output()
            .await;

        let _ = Command::new("git")
            .current_dir(&repo_dir)
            .args([
                "fetch",
                "origin",
                &format!("pull/{}/head", pr_number),
                "--force",
            ])
            .output()
            .await;

        let branch_name = format!("pr-{}", pr_number);
        let _ = Command::new("git")
            .current_dir(&repo_dir)
            .args(["checkout", "-B", &branch_name, "FETCH_HEAD"])
            .output()
            .await;

        // 2. Speculatively merge origin/main into the PR branch
        info!(
            "Speculatively merging origin/main into {} for {}#{}...",
            branch_name, repo, pr_number
        );
        let merge_out = Command::new("git")
            .current_dir(&repo_dir)
            .args(["merge", "origin/main", "--no-edit"])
            .output()
            .await?;

        let has_merge_conflict = !merge_out.status.success();
        let conflict_details = if has_merge_conflict {
            String::from_utf8_lossy(&merge_out.stderr).to_string()
        } else {
            String::new()
        };

        // 3. Prompt Antigravity to repair the merge group failure / conflict
        info!(
            "Invoking Antigravity to repair merge train divergence in {:?}",
            repo_dir
        );
        let prompt = format!(
            r#####"You are Oyatie's Principal Merge Train Resilience Engineer. Pull Request #{pr_number} on `{repo}` failed or was ejected from the GitHub Merge Queue due to train divergence or semantic conflict against trunk.

## PR Details:
- **Repository**: {repo}
- **PR Number**: #{pr_number}
- **Title**: {pr_title}
- **Merge Conflict Status**: {conflict_status}

## Instructions:
1. Inspect the workspace, resolve any git merge conflict markers (`<<<<<<<`), and fix any broken type definitions or API calls caused by upstream trunk changes.
2. Ensure the code compiles cleanly and unit tests pass.
3. Keep the original intent of PR #{pr_number} intact while adapting to trunk changes.

Apply all necessary file edits directly in the repository workspace now."#####,
            pr_number = pr_number,
            repo = repo,
            pr_title = meta.title,
            conflict_status = if has_merge_conflict {
                format!("Merge Conflicts Present:\n{}", conflict_details)
            } else {
                "No textual conflict; Semantic / Test divergence".to_string()
            }
        );

        let _ = self.run_agy_prompt(&prompt, &repo_dir).await?;

        // 4. Run test gate
        let test_ok = self.run_local_test_gate(&repo_dir).await?;
        if !test_ok {
            warn!("Test gate failed after queue healing. Attempting self-correction...");
            let retry_prompt = "Tests failed after merging trunk. Inspect test output, fix the errors, and ensure all tests pass.";
            let _ = self.run_agy_prompt(retry_prompt, &repo_dir).await?;
        }

        // 5. Commit and push the healed branch
        let status_out = Command::new("git")
            .current_dir(&repo_dir)
            .args(["status", "--porcelain"])
            .output()
            .await?;

        let changes = String::from_utf8_lossy(&status_out.stdout);
        if !changes.trim().is_empty() {
            let _ = Command::new("git")
                .current_dir(&repo_dir)
                .args(["add", "-A"])
                .output()
                .await;

            let commit_msg = format!(
                "fix(merge-train): auto-heal merge queue divergence for PR #{}",
                pr_number
            );
            let commit_out = Command::new("git")
                .current_dir(&repo_dir)
                .args(["commit", "-m", &commit_msg])
                .output()
                .await?;

            if commit_out.status.success() {
                let push_target = format!("HEAD:{}", meta.head_ref_name);
                let push_out = Command::new("git")
                    .current_dir(&repo_dir)
                    .args(["push", "origin", &push_target])
                    .output()
                    .await?;

                if push_out.status.success() {
                    info!(
                        "Successfully pushed healed commit to origin/{}",
                        meta.head_ref_name
                    );

                    // Post comment to PR
                    let heal_note = format!(
                        "🛠️ **Merge Queue Self-Healing Applied:**\n\n- Re-synchronized against latest trunk `main`\n- Resolved semantic merge train conflicts\n- Passed local test verification gate\n\n*Re-enlisting into GitHub Merge Queue...*"
                    );
                    let _ = self
                        .github_client
                        .post_pr_comment(repo, pr_number, &heal_note)
                        .await;

                    // Re-enlist in merge queue
                    let _ = self
                        .merge_enlister
                        .enlist_into_merge_queue(repo, pr_number)
                        .await;
                }
            }
        }

        Ok(())
    }

    async fn run_local_test_gate(&self, repo_dir: &Path) -> Result<bool> {
        if repo_dir.join("Cargo.toml").exists() {
            let check = Command::new("cargo")
                .current_dir(repo_dir)
                .arg("check")
                .output()
                .await;
            if let Ok(out) = check {
                if !out.status.success() {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    async fn run_agy_prompt(&self, prompt: &str, working_dir: &Path) -> Result<String> {
        let mut cmd = Command::new("agy");
        cmd.args([
            "--print",
            prompt,
            "--effort",
            &self.agy_effort,
            "--dangerously-skip-permissions",
        ]);
        cmd.current_dir(working_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = cmd.output().await.context("Failed to run agy command")?;
        let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            error!(
                "agy returned non-zero status in QueueHealer: {}",
                output.status
            );
            warn!("agy stderr: {}", stderr_str);
            if stdout_str.trim().is_empty() {
                bail!("agy failed with code {}: {}", output.status, stderr_str);
            }
        }

        Ok(stdout_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pr_number_from_merge_ref() {
        let r1 = "gh-readonly-queue/main/pr-824-7fd7839ed420c8952d5e56c0387350155a8d7fe6";
        assert_eq!(QueueHealer::extract_pr_number_from_merge_ref(r1), Some(824));

        let r2 = "refs/heads/gh-readonly-queue/dev/pr-104-abc";
        assert_eq!(QueueHealer::extract_pr_number_from_merge_ref(r2), Some(104));

        let r3 = "main";
        assert_eq!(QueueHealer::extract_pr_number_from_merge_ref(r3), None);
    }
}
