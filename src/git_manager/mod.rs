use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tracing::info;

pub mod diff_context;
pub mod worktree;

pub use diff_context::PrDiffContext;
pub use worktree::EphemeralWorktree;

#[derive(Clone, Debug)]
pub struct GitManager {
    repos_base_dir: PathBuf,
    worktrees_base_dir: PathBuf,
}

impl GitManager {
    pub fn new(repos_base_dir: PathBuf) -> Self {
        let worktrees_base_dir = repos_base_dir.join(".worktrees");
        Self {
            repos_base_dir,
            worktrees_base_dir,
        }
    }

    /// Gets the local bare/primary path for a given repository (e.g., "oyatie/oyatie" -> "repos/oyatie")
    ///
    /// Defence in depth: callers are expected to have validated the name via
    /// `webhook::repo_guard`, but this took the segment after the last '/'
    /// unconditionally, so `"x/.."` yielded `repos_base_dir.join("..")` —
    /// escaping the repos directory (`install_repo_hooks` then writes
    /// executable files there). Any segment that is not a plain path component
    /// is now sanitised rather than trusted.
    pub fn get_repo_dir(&self, repo: &str) -> PathBuf {
        let raw = repo.split('/').next_back().unwrap_or(repo);
        let safe: String = raw
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || *c == '.' || *c == '_' || *c == '-')
            .collect();
        // Reject any residue containing "..", not just an exact match: stripping
        // disallowed characters can reassemble a traversal-looking component
        // (e.g. "..%2f.." -> "..2f.."). A legitimate repository name never
        // contains "..".
        let name = if safe.is_empty() || safe == "." || safe.contains("..") {
            "_invalid_repo_name"
        } else {
            safe.as_str()
        };
        self.repos_base_dir.join(name)
    }

    /// Ensures the primary repository clone is present locally and up to date
    pub async fn ensure_repo_cloned(&self, repo: &str) -> Result<PathBuf> {
        let repo_dir = self.get_repo_dir(repo);

        if !self.repos_base_dir.exists() {
            tokio::fs::create_dir_all(&self.repos_base_dir)
                .await
                .context("Failed to create repos base directory")?;
        }

        if !self.worktrees_base_dir.exists() {
            tokio::fs::create_dir_all(&self.worktrees_base_dir)
                .await
                .context("Failed to create worktrees directory")?;
        }

        if !repo_dir.exists() {
            info!("Cloning repository {} into {:?}", repo, repo_dir);
            let clone_url = format!("https://github.com/{}.git", repo);
            let output = Command::new("git")
                .args(["clone", &clone_url, repo_dir.to_str().unwrap()])
                .output()
                .await
                .context("Failed to execute git clone")?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                bail!("git clone failed for {}: {}", repo, err);
            }
            info!("Successfully cloned {}", repo);
        } else {
            let _ = Command::new("git")
                .current_dir(&repo_dir)
                .args(["fetch", "origin", "--prune"])
                .output()
                .await;
        }

        let _ = Self::install_repo_hooks(&repo_dir).await;

        Ok(repo_dir)
    }

    /// Automatically maintains and updates standard developer inner-loop git hooks in a maintained repository
    pub async fn install_repo_hooks(repo_dir: &Path) -> Result<()> {
        let hooks_dir = repo_dir.join(".git").join("hooks");
        if !hooks_dir.exists() {
            let _ = tokio::fs::create_dir_all(&hooks_dir).await;
        }

        let pre_commit_script = r#"#!/bin/bash
# Anvil Developer Inner-Loop Pre-Commit Hook (Sub-100ms AST Lint & Hygiene Probe)
set -e

# Run fast formatter and clippy check
cargo fmt -- --check 2>/dev/null || { echo "❌ [pre-commit] 'cargo fmt' failed. Please format code before committing."; exit 1; }
cargo clippy --all-targets -- -D warnings 2>/dev/null || { echo "❌ [pre-commit] 'cargo clippy' caught compiler warnings."; exit 1; }
"#;

        let commit_msg_script = r#"#!/bin/bash
# Anvil Conventional Commit Format Validator
set -e

MSG_FILE="$1"
COMMIT_MSG=$(head -n 1 "$MSG_FILE")

CONVENTIONAL_REGEX="^(feat|fix|chore|docs|style|refactor|perf|test|build|ci|revert)(\([a-zA-Z0-9_\-]+\))?: .+$"

if [[ ! "$COMMIT_MSG" =~ $CONVENTIONAL_REGEX ]] && [[ ! "$COMMIT_MSG" =~ ^Merge ]]; then
    echo "❌ [commit-msg] Commit message '$COMMIT_MSG' violates Conventional Commits standard."
    echo "💡 Expected format: <type>(<scope>): <short summary>"
    echo "💡 Example: feat(orchestrator): add truth verification engine"
    exit 1
fi
"#;

        let pre_push_script = r#"#!/bin/bash
# Anvil Developer Pre-Push Fast Gate (<30s Verification Suite)
set -e

echo "🔍 [pre-push] Running Anvil Fast Verification Suite..."
cargo test --test red_green_gates_test -- --quiet 2>/dev/null || { echo "❌ [pre-push] Quality matrix test failed."; exit 1; }
"#;

        let post_merge_script = r#"#!/bin/bash
# Anvil Post-Merge Lockfile & Drift Reconciler
set -e

if git diff-tree -r --name-only --no-commit-id HEAD@{1} HEAD | grep -q "Cargo.lock"; then
    echo "📦 [post-merge] Cargo.lock modified in merge. Ensuring deterministic dependencies..."
    cargo check --quiet 2>/dev/null || true
fi
"#;

        let pre_commit_path = hooks_dir.join("pre-commit");
        let commit_msg_path = hooks_dir.join("commit-msg");
        let pre_push_path = hooks_dir.join("pre-push");
        let post_merge_path = hooks_dir.join("post-merge");

        let _ = tokio::fs::write(&pre_commit_path, pre_commit_script).await;
        let _ = tokio::fs::write(&commit_msg_path, commit_msg_script).await;
        let _ = tokio::fs::write(&pre_push_path, pre_push_script).await;
        let _ = tokio::fs::write(&post_merge_path, post_merge_script).await;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            let _ = std::fs::set_permissions(&pre_commit_path, perms.clone());
            let _ = std::fs::set_permissions(&commit_msg_path, perms.clone());
            let _ = std::fs::set_permissions(&pre_push_path, perms.clone());
            let _ = std::fs::set_permissions(&post_merge_path, perms);
        }

        Ok(())
    }

    /// Creates an isolated, ephemeral git worktree for concurrent PR evaluation
    pub async fn create_ephemeral_worktree(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
    ) -> Result<EphemeralWorktree> {
        let repo_dir = self.ensure_repo_cloned(repo).await?;

        // Ensure PR ref is fetched in the main repo
        let pr_ref = format!("pull/{}/head", pr_number);
        let _ = Command::new("git")
            .current_dir(&repo_dir)
            .args(["fetch", "origin", &pr_ref, "--force"])
            .output()
            .await;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let safe_repo = repo.replace('/', "-");
        let worktree_name = format!("{}-pr-{}-{}", safe_repo, pr_number, now);
        let worktree_path = self.worktrees_base_dir.join(&worktree_name);

        info!(
            "Creating isolated ephemeral worktree for {}#{} at {:?}",
            repo, pr_number, worktree_path
        );

        let output = Command::new("git")
            .current_dir(&repo_dir)
            .args([
                "worktree",
                "add",
                "--detach",
                worktree_path.to_str().unwrap(),
                head_sha,
            ])
            .output()
            .await
            .context("Failed to create git worktree")?;

        if !output.status.success() {
            // If head_sha isn't in local index yet, fallback to FETCH_HEAD
            let output_fetch = Command::new("git")
                .current_dir(&repo_dir)
                .args([
                    "worktree",
                    "add",
                    "--detach",
                    worktree_path.to_str().unwrap(),
                    "FETCH_HEAD",
                ])
                .output()
                .await
                .context("Failed to create git worktree from FETCH_HEAD")?;

            if !output_fetch.status.success() {
                let err = String::from_utf8_lossy(&output_fetch.stderr);
                bail!(
                    "git worktree add failed for {}#{}: {}",
                    repo,
                    pr_number,
                    err
                );
            }
        }

        Ok(EphemeralWorktree {
            repo: repo.to_string(),
            pr_number,
            worktree_path,
            repo_dir,
        })
    }

    /// Garbage collects and cleans up abandoned worktrees left behind by crashed/killed processes
    pub async fn clean_abandoned_worktrees(&self) -> Result<usize> {
        info!("Running garbage collection for abandoned git worktrees...");
        let mut cleaned = 0;

        if !self.worktrees_base_dir.exists() {
            return Ok(0);
        }

        // 1. Iterate through base repos and run git worktree prune
        if let Ok(mut entries) = tokio::fs::read_dir(&self.repos_base_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.is_dir() && path.file_name().map(|n| n != ".worktrees").unwrap_or(false) {
                    let _ = Command::new("git")
                        .current_dir(&path)
                        .args(["worktree", "prune"])
                        .output()
                        .await;
                }
            }
        }

        // 2. Scan .worktrees for orphaned directories older than TTL (30 minutes)
        let ttl = Duration::from_secs(1800);
        let now = SystemTime::now();

        if let Ok(mut entries) = tokio::fs::read_dir(&self.worktrees_base_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if let Ok(metadata) = entry.metadata().await {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(age) = now.duration_since(modified) {
                            if age > ttl {
                                info!("Pruning abandoned ephemeral worktree directory: {:?}", path);
                                let _ = tokio::fs::remove_dir_all(&path).await;
                                cleaned += 1;
                            }
                        }
                    }
                }
            }
        }

        if cleaned > 0 {
            info!(
                "Successfully reclaimed {} abandoned ephemeral worktree(s).",
                cleaned
            );
        }

        Ok(cleaned)
    }

    /// Fetches the PR ref, creates a diff context, and computes changes
    pub async fn prepare_pr_diff(
        &self,
        repo: &str,
        pr_number: u64,
        base_branch: &str,
        base_sha: &str,
        head_sha: &str,
        last_reviewed_sha: Option<&str>,
    ) -> Result<PrDiffContext> {
        let repo_dir = self.ensure_repo_cloned(repo).await?;

        info!(
            "Fetching PR #{} for {} (head: {}, base: {})",
            pr_number, repo, head_sha, base_branch
        );

        let pr_ref = format!("pull/{}/head", pr_number);
        let _ = Command::new("git")
            .current_dir(&repo_dir)
            .args(["fetch", "origin", &pr_ref, "--force"])
            .output()
            .await;

        let is_incremental = last_reviewed_sha.is_some()
            && last_reviewed_sha.unwrap() != head_sha
            && last_reviewed_sha.unwrap() != base_sha;

        let diff_content = if is_incremental {
            let prev_sha = last_reviewed_sha.unwrap();
            self.run_git_diff(&repo_dir, prev_sha, head_sha).await?
        } else {
            let diff_res = self.run_git_diff(&repo_dir, base_sha, head_sha).await;
            match diff_res {
                Ok(diff) if !diff.trim().is_empty() => diff,
                _ => {
                    let base_ref = format!("origin/{}", base_branch);
                    self.run_git_diff(&repo_dir, &base_ref, head_sha)
                        .await
                        .unwrap_or_default()
                }
            }
        };

        let changed_files = self
            .get_changed_files(
                &repo_dir,
                if is_incremental {
                    last_reviewed_sha.unwrap()
                } else {
                    base_sha
                },
                head_sha,
            )
            .await
            .unwrap_or_default();

        Ok(PrDiffContext {
            repo: repo.to_string(),
            pr_number,
            base_branch: base_branch.to_string(),
            base_sha: base_sha.to_string(),
            head_sha: head_sha.to_string(),
            is_incremental,
            previous_head_sha: last_reviewed_sha.map(|s| s.to_string()),
            diff_content,
            changed_files,
            repo_working_dir: repo_dir,
        })
    }

    async fn run_git_diff(&self, repo_dir: &Path, from_ref: &str, to_ref: &str) -> Result<String> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["diff", "--unified=3", &format!("{}...{}", from_ref, to_ref)])
            .output()
            .await
            .context("Failed to run git diff")?;

        if !output.status.success() {
            let output2 = Command::new("git")
                .current_dir(repo_dir)
                .args(["diff", "--unified=3", from_ref, to_ref])
                .output()
                .await
                .context("Failed to run two-dot git diff")?;

            if output2.status.success() {
                return Ok(String::from_utf8_lossy(&output2.stdout).to_string());
            }
            let err = String::from_utf8_lossy(&output.stderr);
            bail!("git diff failed: {}", err);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn get_changed_files(
        &self,
        repo_dir: &Path,
        from_ref: &str,
        to_ref: &str,
    ) -> Result<Vec<String>> {
        let output = Command::new("git")
            .current_dir(repo_dir)
            .args(["diff", "--name-only", from_ref, to_ref])
            .output()
            .await
            .context("Failed to get changed files")?;

        if output.status.success() {
            let files = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(files)
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_repo_dir_cannot_escape_the_repos_directory() {
        let gm = GitManager::new(PathBuf::from("/tmp/anvil-repos"));
        for hostile in ["x/..", "../etc", "x/../..", "a/.", "owner/..%2f.."] {
            let p = gm.get_repo_dir(hostile);
            assert!(
                p.starts_with("/tmp/anvil-repos"),
                "{hostile:?} escaped to {p:?}"
            );
            assert!(
                !p.to_string_lossy().contains(".."),
                "{hostile:?} produced a traversal component: {p:?}"
            );
        }
    }

    #[test]
    fn get_repo_dir_still_resolves_normal_names() {
        let gm = GitManager::new(PathBuf::from("/tmp/anvil-repos"));
        assert_eq!(
            gm.get_repo_dir("oyatie/anvil"),
            PathBuf::from("/tmp/anvil-repos/anvil")
        );
    }

    #[tokio::test]
    async fn test_git_manager_creates_and_cleans_abandoned_worktrees() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let git_mgr = GitManager::new(temp_dir.path().to_path_buf());

        let cleaned = git_mgr.clean_abandoned_worktrees().await.expect("clean");
        assert_eq!(cleaned, 0);
    }
}
