use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tokio::process::Command;
use tracing::{info, warn};

pub mod diff_context;
pub mod worktree;

pub use diff_context::PrDiffContext;
pub use worktree::EphemeralWorktree;

/// Paths Anvil writes into somebody else's checkout. A commit Anvil pushes
/// carries what the change produced, never Anvil's own bookkeeping
/// (`.cursor/receipts` is the legacy location, still present in older
/// checkouts).
const ANVIL_OWNED_PATHS: &[&str] = &[
    crate::attestation_guard::ANVIL_RECEIPTS_DIR,
    ".cursor/receipts",
];

/// The `git add` that every Anvil staging site runs: stage the whole tree
/// except Anvil's own bookkeeping.
///
/// Returns the built `Command` rather than its arguments. Four sites each
/// spelled their own `["add", "-A"]` -- `certify.rs` did it sixteen lines under
/// a comment saying it must never do that -- and only `QueueHealer` carried the
/// exclusion, so three of them committed Anvil's receipt onto the pull request
/// it had just written the receipt into. Handing back a `Vec` was not enough:
/// a caller can take the arguments and pass `&args[..2]`, which is the bug
/// again with the shared function's name on it.
pub fn stage_excluding_receipts(repo_dir: &std::path::Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(repo_dir).args(["add", "-A", "--", "."]);
    for p in ANVIL_OWNED_PATHS {
        cmd.arg(format!(":(exclude){p}"));
    }
    cmd
}

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
            let mut clone_cmd = Command::new("git");
            clone_cmd.args(["clone", &clone_url, repo_dir.to_str().unwrap()]);
            let output =
                crate::exec::run_bounded(clone_cmd, crate::exec::ExecClass::Vcs, "git clone")
                    .await
                    .context("Failed to execute git clone")?;

            if !output.status.success() {
                let err = String::from_utf8_lossy(&output.stderr);
                bail!("git clone failed for {}: {}", repo, err);
            }
            info!("Successfully cloned {}", repo);
        } else {
            let mut fetch_cmd = Command::new("git");
            fetch_cmd
                .current_dir(&repo_dir)
                .args(["fetch", "origin", "--prune"]);
            let _ = crate::exec::run_bounded(
                fetch_cmd,
                crate::exec::ExecClass::Vcs,
                "git fetch origin --prune",
            )
            .await;
        }

        let _ = Self::install_repo_hooks(&repo_dir).await;

        Ok(repo_dir)
    }

    /// Native hooks live in `$(git rev-parse --git-common-dir)/hooks`.
    /// Worktrees share that directory. `core.hooksPath` stays unset.
    fn common_hooks_dir(repo_dir: &Path) -> Result<PathBuf> {
        let out = std::process::Command::new("git")
            .args(["-C"])
            .arg(repo_dir)
            .args(["rev-parse", "--git-common-dir"])
            .output()
            .context("git rev-parse --git-common-dir")?;
        if !out.status.success() {
            bail!(
                "git-common-dir failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        let common = String::from_utf8(out.stdout)?.trim().to_string();
        let common_path = Path::new(&common);
        let common_path = if common_path.is_absolute() {
            common_path.to_path_buf()
        } else {
            repo_dir.join(common_path)
        };
        Ok(common_path.join("hooks"))
    }

    /// Copies the crate-owned hook templates into the common hooks directory
    /// and leaves `core.hooksPath` unset.
    pub async fn install_repo_hooks(repo_dir: &Path) -> Result<()> {
        let hooks_dir = Self::common_hooks_dir(repo_dir)?;
        tokio::fs::create_dir_all(&hooks_dir)
            .await
            .with_context(|| format!("create {}", hooks_dir.display()))?;

        let templates = [
            ("pre-commit", include_str!("hooks/pre-commit")),
            ("commit-msg", include_str!("hooks/commit-msg")),
            ("pre-push", include_str!("hooks/pre-push")),
        ];
        for (name, body) in templates {
            let path = hooks_dir.join(name);
            tokio::fs::write(&path, body)
                .await
                .with_context(|| format!("write {}", path.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                    .with_context(|| format!("chmod {}", path.display()))?;
            }
        }

        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("-C")
            .arg(repo_dir)
            .args(["config", "--unset-all", "core.hooksPath"])
            .stdin(std::process::Stdio::null());
        let _ = crate::exec::run_bounded(
            cmd,
            crate::exec::ExecClass::Vcs,
            "git config unset hooksPath",
        )
        .await;
        Ok(())
    }

    /// Creates an isolated, ephemeral git worktree for concurrent PR evaluation.
    ///
    /// `head_sha` is what is *asked for*, and the returned worktree may not be
    /// at it: when the object is not local the `FETCH_HEAD` fallback below
    /// checks out whatever ref the shared clone fetched last, and that clone is
    /// shared across every pull request of the repository -- a concurrent
    /// `prepare_pr_diff` for a different pull request, or the
    /// `git fetch origin <base> --prune` the queue healer runs immediately
    /// before calling this, moves it. Callers that are going to describe the
    /// result as a particular commit must say so with
    /// `EphemeralWorktree::verify_at`, which is what makes the tree evidence
    /// about that commit rather than about the clone's last fetch.
    pub async fn create_ephemeral_worktree(
        &self,
        repo: &str,
        pr_number: u64,
        head_sha: &str,
    ) -> Result<EphemeralWorktree> {
        let repo_dir = self.ensure_repo_cloned(repo).await?;

        // Ensure PR ref is fetched in the main repo. A failure here is not
        // fatal -- `head_sha` may already be local -- but it is not nothing
        // either: it is what makes the `FETCH_HEAD` fallback below reachable,
        // and the fallback is what checks out a commit nobody asked for. Logged
        // rather than swallowed, so the reason a `verify_at` refusal happened is
        // in the same log as the refusal.
        let pr_ref = format!("pull/{}/head", pr_number);
        let mut fetch_ref_cmd = Command::new("git");
        fetch_ref_cmd
            .current_dir(&repo_dir)
            .args(["fetch", "origin", &pr_ref, "--force"]);
        match crate::exec::run_bounded(
            fetch_ref_cmd,
            crate::exec::ExecClass::Vcs,
            "git fetch pull request ref",
        )
        .await
        {
            Ok(out) if out.status.success() => {}
            Ok(out) => warn!(
                "git fetch origin {} did not succeed for {}#{}: {}",
                pr_ref,
                repo,
                pr_number,
                String::from_utf8_lossy(&out.stderr).trim()
            ),
            Err(e) => warn!(
                "git fetch origin {} did not complete for {}#{}: {:#}",
                pr_ref, repo, pr_number, e
            ),
        }

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

        let mut worktree_cmd = Command::new("git");
        worktree_cmd.current_dir(&repo_dir).args([
            "worktree",
            "add",
            "--detach",
            worktree_path.to_str().unwrap(),
            head_sha,
        ]);
        let output = crate::exec::run_bounded(
            worktree_cmd,
            crate::exec::ExecClass::Vcs,
            "git worktree add",
        )
        .await
        .context("Failed to create git worktree")?;

        if !output.status.success() {
            // If head_sha isn't in local index yet, fallback to FETCH_HEAD
            let mut worktree_fetch_cmd = Command::new("git");
            worktree_fetch_cmd.current_dir(&repo_dir).args([
                "worktree",
                "add",
                "--detach",
                worktree_path.to_str().unwrap(),
                "FETCH_HEAD",
            ]);
            let output_fetch = crate::exec::run_bounded(
                worktree_fetch_cmd,
                crate::exec::ExecClass::Vcs,
                "git worktree add FETCH_HEAD",
            )
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
                    let mut prune_cmd = Command::new("git");
                    prune_cmd.current_dir(&path).args(["worktree", "prune"]);
                    let _ = crate::exec::run_bounded(
                        prune_cmd,
                        crate::exec::ExecClass::Quick,
                        "git worktree prune",
                    )
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
                if let Ok(metadata) = entry.metadata().await
                    && let Ok(modified) = metadata.modified()
                    && let Ok(age) = now.duration_since(modified)
                    && age > ttl
                {
                    // A live lane holds an unexpired lease; mtime alone would
                    // reap a lane mid-build (mechanism, not "remember to
                    // touch the directory").
                    if lane_lease_unexpired(&path).await {
                        continue;
                    }
                    info!("Pruning abandoned ephemeral worktree directory: {:?}", path);
                    let _ = tokio::fs::remove_dir_all(&path).await;
                    cleaned += 1;
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

        // Propagated, not swallowed. Discarded, a failed or rate-limited fetch
        // fell through to `unwrap_or_default()` below and returned a context
        // with an empty diff and no changed files: every diff-scanning gate
        // then passed on nothing, and the corpus stamped a fully certified
        // report over zero measured lines. "The diff could not be obtained" is
        // an error, not a measurement of no changes.
        let pr_ref = format!("pull/{}/head", pr_number);
        let mut fetch_ref_cmd = Command::new("git");
        fetch_ref_cmd
            .current_dir(&repo_dir)
            .args(["fetch", "origin", &pr_ref, "--force"]);
        let fetch_out = crate::exec::run_bounded(
            fetch_ref_cmd,
            crate::exec::ExecClass::Vcs,
            "git fetch pull request ref",
        )
        .await
        .context("Failed to run git fetch for the pull request ref")?;
        if !fetch_out.status.success() {
            bail!(
                "git fetch origin {} failed for {}: {}",
                pr_ref,
                repo,
                String::from_utf8_lossy(&fetch_out.stderr).trim()
            );
        }

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
                        .with_context(|| {
                            format!(
                                "no diff could be computed for {}#{} between {} and {}",
                                repo, pr_number, base_ref, head_sha
                            )
                        })?
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
            .with_context(|| {
                format!(
                    "the changed-file list could not be read for {}#{} at {}",
                    repo, pr_number, head_sha
                )
            })?;

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
        let mut diff_cmd = Command::new("git");
        diff_cmd.current_dir(repo_dir).args([
            "diff",
            "--unified=3",
            &format!("{}...{}", from_ref, to_ref),
        ]);
        let output = crate::exec::run_bounded(
            diff_cmd,
            crate::exec::ExecClass::Quick,
            "git diff (three-dot)",
        )
        .await
        .context("Failed to run git diff")?;

        if !output.status.success() {
            let mut diff2_cmd = Command::new("git");
            diff2_cmd
                .current_dir(repo_dir)
                .args(["diff", "--unified=3", from_ref, to_ref]);
            let output2 = crate::exec::run_bounded(
                diff2_cmd,
                crate::exec::ExecClass::Quick,
                "git diff (two-dot)",
            )
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
        let mut names_cmd = Command::new("git");
        names_cmd
            .current_dir(repo_dir)
            .args(["diff", "--name-only", from_ref, to_ref]);
        let output = crate::exec::run_bounded(
            names_cmd,
            crate::exec::ExecClass::Quick,
            "git diff --name-only",
        )
        .await
        .context("Failed to get changed files")?;

        if !output.status.success() {
            // An unreadable file list is not an empty file list. Returned as
            // `Ok(vec![])` it read as "this pull request changes nothing", which
            // is the shape every diff-scanning gate passes on.
            bail!(
                "git diff --name-only {}..{} failed: {}",
                from_ref,
                to_ref,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        Ok(files)
    }
}

/// True when `dir` holds a lane lease naming a future expiry (epoch seconds).
/// An unreadable or malformed lease does not protect the directory.
async fn lane_lease_unexpired(dir: &std::path::Path) -> bool {
    let lease = dir.join(crate::change_delivery::adapters::git_vcs::LANE_LEASE_FILE);
    let Ok(raw) = tokio::fs::read_to_string(&lease).await else {
        return false;
    };
    let Ok(expiry) = raw.trim().parse::<u64>() else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    expiry > now
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

    /// The exclusion has to name every path Anvil owns, not just the current
    /// one: a checkout carried over from before the move still has the legacy
    /// directory, and staging that is the same defect.
    #[test]
    fn the_staging_command_excludes_every_path_anvil_owns() {
        let cmd = stage_excluding_receipts(Path::new("/tmp"));
        let args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&args[..4], &["add", "-A", "--", "."]);
        for p in ANVIL_OWNED_PATHS {
            assert!(
                args.contains(&format!(":(exclude){p}")),
                "{p} would be staged into somebody else's commit: {args:?}"
            );
        }
    }

    #[tokio::test]
    async fn install_repo_hooks_writes_common_dir_and_leaves_hooks_path_unset() {
        let dir = tempfile::tempdir().expect("tempdir");
        let repo = dir.path();
        let init = std::process::Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .expect("git init");
        assert!(init.status.success(), "git init failed: {init:?}");

        GitManager::install_repo_hooks(repo)
            .await
            .expect("install_repo_hooks");

        let common = std::process::Command::new("git")
            .args(["-C"])
            .arg(repo)
            .args(["rev-parse", "--git-common-dir"])
            .output()
            .expect("git-common-dir");
        let common = String::from_utf8_lossy(&common.stdout).trim().to_string();
        let hooks = if std::path::Path::new(&common).is_absolute() {
            std::path::PathBuf::from(&common).join("hooks")
        } else {
            repo.join(&common).join("hooks")
        };
        for name in ["pre-commit", "commit-msg", "pre-push"] {
            let p = hooks.join(name);
            assert!(p.is_file(), "missing {}", p.display());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&p)
                    .expect("metadata")
                    .permissions()
                    .mode();
                assert!(
                    mode & 0o111 != 0,
                    "{} not executable ({mode:o})",
                    p.display()
                );
            }
        }

        let out = std::process::Command::new("git")
            .args(["-C"])
            .arg(repo)
            .args(["config", "--local", "--get", "core.hooksPath"])
            .output()
            .expect("git config");
        assert!(
            !out.status.success(),
            "core.hooksPath must stay unset; got {}",
            String::from_utf8_lossy(&out.stdout)
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
