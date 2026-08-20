use anyhow::{bail, Context, Result};
use regex::Regex;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tracing::{error, info, warn};

pub mod bisector;
pub use bisector::{BisectionResult, MergeTrainBisector};

use crate::attestation_guard::ANVIL_RECEIPTS_DIR;
use crate::git_manager::GitManager;
use crate::github::{GitHubClient, PrMetadata};
use crate::merge_enlister::MergeEnlister;

/// Upper bound for one agy repair turn, matching `ExecClass::Model`.
///
/// agy's own `--print-timeout` defaults to 5m0s and fires with
/// `Error: timeout waiting for response` (exit 1) regardless of how long Anvil
/// is willing to wait. The healer therefore passes an explicit `--print-timeout`
/// a little under its own kill so the two deadlines agree and agy's default
/// never silently wins.
const AGY_TURN_LIMIT: Duration = Duration::from_secs(600);
const AGY_PRINT_TIMEOUT_MARGIN: Duration = Duration::from_secs(30);

/// Receipts Anvil writes into a checkout. A heal commit carries what the repair
/// produced, never Anvil's own provenance artifacts (`.cursor/receipts` is the
/// legacy location, still present in older checkouts).
const ANVIL_OWNED_PATHS: &[&str] = &[ANVIL_RECEIPTS_DIR, ".cursor/receipts"];

/// Outcome of the local verification gate.
///
/// `Unavailable` is not a pass: a repository without a gate Anvil knows how to
/// run gets no heal pushed, because the heal note would otherwise claim a
/// verification that never happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestGate {
    Passed(&'static str),
    Failed(&'static str),
    Unavailable,
}

/// Result policy for a model turn that edits the workspace: any non-zero exit
/// is a failed turn. Partial stdout from a process that died mid-edit is not a
/// partial repair; it is a tree in a state nobody chose. Shared with
/// `fixer::engine`, which has the same shape and failed the same way.
pub fn interpret_agy_outcome(status_success: bool, stdout: &str, stderr: &str) -> Result<String> {
    if !status_success {
        let why = stderr.trim();
        if why.is_empty() {
            bail!("agy exited non-zero with no stderr");
        }
        bail!("agy exited non-zero: {}", why);
    }
    Ok(stdout.to_string())
}

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

    /// Only an open PR can be healed. `merge_group destroyed` also fires when a
    /// group is dequeued because it merged, so the healer must check rather than
    /// trust the trigger.
    pub fn pr_is_healable(state: &str) -> bool {
        state.trim().eq_ignore_ascii_case("open")
    }

    /// Value for agy's `--print-timeout` (Go duration syntax) given Anvil's bound.
    pub fn agy_print_timeout_arg(limit: Duration) -> String {
        let secs = limit
            .saturating_sub(AGY_PRINT_TIMEOUT_MARGIN)
            .as_secs()
            .max(1);
        format!("{}s", secs)
    }

    /// Arguments for `git add` that stage the repair but exclude Anvil's own
    /// receipts, so a stray attestation never becomes a "healed" commit.
    pub fn heal_add_args() -> Vec<String> {
        let mut args = vec![
            "add".to_string(),
            "-A".to_string(),
            "--".to_string(),
            ".".to_string(),
        ];
        for p in ANVIL_OWNED_PATHS {
            args.push(format!(":(exclude){}", p));
        }
        args
    }

    /// Comment body for a pushed heal; says only what was actually done.
    pub fn heal_note(base_branch: &str, gate: &TestGate) -> String {
        let gate_line = match gate {
            TestGate::Passed(label) => format!("- Local gate `{}` passed", label),
            // Unreachable for a pushed heal; spelled out so the note never lies
            // if the call site changes.
            TestGate::Failed(label) => format!("- Local gate `{}` FAILED", label),
            TestGate::Unavailable => "- No local gate available (not verified)".to_string(),
        };
        format!(
            "🛠️ **Merge Queue Self-Healing Applied:**\n\n\
             - Re-synchronized against latest trunk `{}`\n\
             - Merge train divergence repaired by Antigravity\n\
             {}\n\n\
             *Re-enlisting into GitHub Merge Queue...*\n\n---\n*🤖 [Healed] by Oyatie Anvil*",
            base_branch, gate_line
        )
    }

    /// Heals an ejected or failed merge queue PR
    pub async fn heal_ejected_pr(&self, repo: &str, pr_number: u64) -> Result<()> {
        info!("Starting Merge Queue Healer for {}#{}...", repo, pr_number);

        let meta = self
            .github_client
            .fetch_pr_metadata(repo, pr_number)
            .await?;

        if !Self::pr_is_healable(&meta.state) {
            info!(
                "Skipping queue heal for {}#{}: PR state is {}, not OPEN",
                repo, pr_number, meta.state
            );
            return Ok(());
        }

        let base_branch = if meta.base_ref_name.trim().is_empty() {
            "dev".to_string()
        } else {
            meta.base_ref_name.clone()
        };

        // 1. Fetch latest base branch into the shared clone; the worktree below
        //    shares its refs.
        let repo_dir = self.git_mgr.ensure_repo_cloned(repo).await?;
        let mut fetch_base_cmd = Command::new("git");
        fetch_base_cmd
            .current_dir(&repo_dir)
            .args(["fetch", "origin", &base_branch, "--prune"]);
        let _ = crate::exec::run_bounded(
            fetch_base_cmd,
            crate::exec::ExecClass::Vcs,
            "git fetch origin base (queue healer)",
        )
        .await;

        // 2. Work in an isolated worktree at the PR head. The shared clone
        //    carries other stages' in-flight state (receipts, checked-out
        //    branches) that must not be swept into a heal commit.
        let worktree = self
            .git_mgr
            .create_ephemeral_worktree(repo, pr_number, &meta.head_ref_oid)
            .await?;
        let result = self
            .heal_in_worktree(
                repo,
                pr_number,
                &meta,
                &base_branch,
                &worktree.worktree_path,
            )
            .await;
        if let Err(e) = worktree.cleanup().await {
            warn!(
                "Queue healer worktree cleanup failed for {}#{}: {}",
                repo, pr_number, e
            );
        }
        result
    }

    async fn heal_in_worktree(
        &self,
        repo: &str,
        pr_number: u64,
        meta: &PrMetadata,
        base_branch: &str,
        work_dir: &Path,
    ) -> Result<()> {
        // 3. Speculatively merge origin/<base_branch> into the PR head
        info!(
            "Speculatively merging origin/{} into pr-{} for {}#{}...",
            base_branch, pr_number, repo, pr_number
        );
        let mut merge_cmd = Command::new("git");
        merge_cmd.current_dir(work_dir).args([
            "merge",
            &format!("origin/{}", base_branch),
            "--no-edit",
        ]);
        let merge_out = crate::exec::run_bounded(
            merge_cmd,
            crate::exec::ExecClass::Vcs,
            "git merge origin/base (queue healer)",
        )
        .await?;

        let has_merge_conflict = !merge_out.status.success();
        let conflict_details = if has_merge_conflict {
            String::from_utf8_lossy(&merge_out.stderr).to_string()
        } else {
            String::new()
        };

        // 4. Prompt Antigravity to repair the merge group failure / conflict
        info!(
            "Invoking Antigravity to repair merge train divergence in {:?}",
            work_dir
        );
        let prompt = format!(
            r#####"You are Oyatie's Principal Merge Train Resilience Engineer. Pull Request #{pr_number} on `{repo}` failed or was ejected from the GitHub Merge Queue due to train divergence or semantic conflict against trunk.

**Context:**
- **Repository**: {repo}
- **Base Branch**: {base_branch}
- **PR Head Branch**: {head_ref}
- **Merge Conflict Status**: {conflict_status}

**Task:**
1. Inspect the workspace, resolve any git merge conflict markers (`<<<<<<<`), and fix any broken type definitions or API calls caused by upstream trunk changes.
2. Ensure the codebase compiles and passes all tests.
3. Do NOT commit; leave your changes in the working tree.
"#####,
            pr_number = pr_number,
            repo = repo,
            base_branch = base_branch,
            head_ref = meta.head_ref_name,
            conflict_status = if has_merge_conflict {
                format!("Merge Conflicts Present:\n{}", conflict_details)
            } else {
                "No textual conflict; Semantic / Test divergence".to_string()
            }
        );

        self.run_agy_prompt(&prompt, work_dir).await?;

        // 5. Run the local gate; one self-correction turn on failure
        let mut gate = self.run_local_test_gate(work_dir).await;
        if let TestGate::Failed(label) = gate {
            warn!(
                "Gate `{}` failed after queue healing for {}#{}. Attempting self-correction...",
                label, repo, pr_number
            );
            let retry_prompt = "Tests failed after merging trunk. Inspect test output, fix the errors, and ensure all tests pass. Do NOT commit.";
            self.run_agy_prompt(retry_prompt, work_dir).await?;
            gate = self.run_local_test_gate(work_dir).await;
        }
        match &gate {
            TestGate::Passed(label) => info!("Gate `{}` passed for {}#{}", label, repo, pr_number),
            TestGate::Failed(label) => {
                bail!(
                    "Queue heal for {}#{} not pushed: gate `{}` still failing after self-correction",
                    repo,
                    pr_number,
                    label
                );
            }
            TestGate::Unavailable => {
                bail!(
                    "Queue heal for {}#{} not pushed: no local test gate Anvil can run in this repository (needs a root Cargo.toml or a package.json `test` script)",
                    repo,
                    pr_number
                );
            }
        }

        // 6. Stage the repair, excluding Anvil's own receipts
        let mut add_cmd = Command::new("git");
        add_cmd.current_dir(work_dir).args(Self::heal_add_args());
        let add_out = crate::exec::run_bounded(
            add_cmd,
            crate::exec::ExecClass::Quick,
            "git add (queue healer)",
        )
        .await?;
        if !add_out.status.success() {
            bail!(
                "git add failed in queue healer for {}#{}: {}",
                repo,
                pr_number,
                String::from_utf8_lossy(&add_out.stderr).trim()
            );
        }

        let mut staged_cmd = Command::new("git");
        staged_cmd
            .current_dir(work_dir)
            .args(["diff", "--cached", "--quiet"]);
        let staged_out = crate::exec::run_bounded(
            staged_cmd,
            crate::exec::ExecClass::Quick,
            "git diff --cached --quiet (queue healer)",
        )
        .await?;
        if staged_out.status.success() {
            info!(
                "Queue heal for {}#{} produced no changes to push",
                repo, pr_number
            );
            return Ok(());
        }

        // A staged conflict marker means the repair did not finish.
        let mut marker_cmd = Command::new("git");
        marker_cmd.current_dir(work_dir).args([
            "diff",
            "--cached",
            "--name-only",
            "-G^(<<<<<<< |>>>>>>> )",
        ]);
        let marker_out = crate::exec::run_bounded(
            marker_cmd,
            crate::exec::ExecClass::Quick,
            "git diff --cached -G conflict markers (queue healer)",
        )
        .await?;
        let marker_files: Vec<&str> = std::str::from_utf8(&marker_out.stdout)
            .unwrap_or_default()
            .lines()
            .filter(|l| !l.is_empty())
            .collect();
        if !marker_files.is_empty() {
            bail!(
                "Queue heal for {}#{} not pushed: conflict markers remain in {}",
                repo,
                pr_number,
                marker_files.join(", ")
            );
        }

        // 7. Commit. Hooks may run formatters or clippy, so this is a Build-class bound.
        let commit_msg = format!(
            "fix(merge-train): auto-heal merge queue divergence for PR #{}\n\n\
             X-Anvil-Action: queue-heal\n\
             X-Anvil-Version: 0.1.0\n\n\
             *🤖 [Healed] by Oyatie Anvil*",
            pr_number
        );
        let mut commit_cmd = Command::new("git");
        commit_cmd
            .current_dir(work_dir)
            .args(["commit", "-m", &commit_msg]);
        let commit_out = crate::exec::run_bounded(
            commit_cmd,
            crate::exec::ExecClass::Build,
            "git commit (queue healer)",
        )
        .await?;
        if !commit_out.status.success() {
            bail!(
                "git commit failed in queue healer for {}#{}: {}",
                repo,
                pr_number,
                String::from_utf8_lossy(&commit_out.stderr).trim()
            );
        }

        // 8. Push. Never push to a branch that belongs to a fork; see github::fork_guard.
        crate::github::fork_guard::ensure_push_allowed(repo, pr_number, meta.is_cross_repository)?;
        let push_target = format!("HEAD:{}", meta.head_ref_name);
        let mut push_cmd = Command::new("git");
        push_cmd
            .current_dir(work_dir)
            .args(["push", "origin", &push_target]);
        let push_out = crate::exec::run_bounded(
            push_cmd,
            crate::exec::ExecClass::Vcs,
            "git push (queue healer)",
        )
        .await?;
        if !push_out.status.success() {
            bail!(
                "git push to origin/{} failed in queue healer for {}#{}: {}",
                meta.head_ref_name,
                repo,
                pr_number,
                String::from_utf8_lossy(&push_out.stderr).trim()
            );
        }
        info!(
            "Successfully pushed healed commit to origin/{}",
            meta.head_ref_name
        );

        // 9. Comment and re-enlist
        let heal_note = Self::heal_note(base_branch, &gate);
        if let Err(e) = self
            .github_client
            .post_pr_comment(repo, pr_number, &heal_note)
            .await
        {
            warn!("Could not post heal note on {}#{}: {}", repo, pr_number, e);
        }
        if let Err(e) = self
            .merge_enlister
            .enlist_into_merge_queue(repo, pr_number)
            .await
        {
            warn!(
                "Could not re-enlist {}#{} after heal: {}",
                repo, pr_number, e
            );
        }

        Ok(())
    }

    /// Picks the gate from what the repository provides and runs it. A gate that
    /// never completed (spawn failure or build timeout) is a failure, not a pass.
    async fn run_local_test_gate(&self, repo_dir: &Path) -> TestGate {
        let (label, mut cmd) = if repo_dir.join("Cargo.toml").exists() {
            let mut c = Command::new("cargo");
            c.arg("check");
            ("cargo check", c)
        } else if Self::has_npm_test_script(repo_dir).await {
            let mut c = Command::new("npm");
            c.args(["test", "--silent"]);
            ("npm test", c)
        } else {
            return TestGate::Unavailable;
        };
        cmd.current_dir(repo_dir);

        match crate::exec::run_bounded(cmd, crate::exec::ExecClass::Build, label).await {
            Ok(out) if out.status.success() => TestGate::Passed(label),
            Ok(out) => {
                warn!(
                    "{} failed in queue healer gate: {}",
                    label,
                    String::from_utf8_lossy(&out.stderr).trim()
                );
                TestGate::Failed(label)
            }
            Err(e) => {
                warn!("{} did not complete in queue healer gate: {}", label, e);
                TestGate::Failed(label)
            }
        }
    }

    async fn has_npm_test_script(repo_dir: &Path) -> bool {
        let Ok(raw) = tokio::fs::read(repo_dir.join("package.json")).await else {
            return false;
        };
        Self::package_json_has_test_script(&raw)
    }

    pub fn package_json_has_test_script(raw: &[u8]) -> bool {
        serde_json::from_slice::<serde_json::Value>(raw)
            .ok()
            .and_then(|v| {
                v.get("scripts")?
                    .get("test")?
                    .as_str()
                    .map(|s| !s.trim().is_empty())
            })
            .unwrap_or(false)
    }

    async fn run_agy_prompt(&self, prompt: &str, working_dir: &Path) -> Result<String> {
        let mut cmd = Command::new("agy");
        cmd.args([
            "--print",
            prompt,
            "--effort",
            &self.agy_effort,
            "--print-timeout",
            &Self::agy_print_timeout_arg(AGY_TURN_LIMIT),
            "--dangerously-skip-permissions",
        ]);
        cmd.current_dir(working_dir);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let output = crate::exec::run_bounded_for(cmd, AGY_TURN_LIMIT, "agy (queue healer)")
            .await
            .context("Failed to run agy command")?;
        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let stderr_str = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            error!(
                "agy returned non-zero status in QueueHealer: {}",
                output.status
            );
            warn!("agy stderr: {}", stderr_str.trim());
        }
        interpret_agy_outcome(output.status.success(), &stdout_str, &stderr_str)
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

    #[test]
    fn agy_failure_is_a_failure_even_with_partial_stdout() {
        // 2026-08-20 13:41:45: agy exited 1 ("timeout waiting for response")
        // after streaming text; the healer treated it as a repair and pushed.
        let r = interpret_agy_outcome(
            false,
            "Inspecting the workspace...\n",
            "Error: timeout waiting for response\n",
        );
        let err = r.expect_err("non-zero agy exit must not be a repair");
        assert!(err.to_string().contains("timeout waiting for response"));

        let ok = interpret_agy_outcome(true, "done", "").unwrap();
        assert_eq!(ok, "done");
    }

    #[test]
    fn agy_print_timeout_sits_under_anvil_bound() {
        assert_eq!(
            QueueHealer::agy_print_timeout_arg(Duration::from_secs(600)),
            "570s"
        );
        // Never emits 0s, which agy would read as "no wait".
        assert_eq!(
            QueueHealer::agy_print_timeout_arg(Duration::from_secs(5)),
            "1s"
        );
    }

    #[test]
    fn only_open_prs_are_healed() {
        assert!(QueueHealer::pr_is_healable("OPEN"));
        assert!(QueueHealer::pr_is_healable("open"));
        assert!(!QueueHealer::pr_is_healable("MERGED"));
        assert!(!QueueHealer::pr_is_healable("CLOSED"));
        assert!(!QueueHealer::pr_is_healable(""));
    }

    #[test]
    fn heal_commit_excludes_anvil_receipts() {
        let args = QueueHealer::heal_add_args();
        assert_eq!(&args[..4], &["add", "-A", "--", "."]);
        assert!(args.contains(&format!(":(exclude){}", ANVIL_RECEIPTS_DIR)));
        assert!(args.contains(&":(exclude).cursor/receipts".to_string()));
    }

    #[test]
    fn heal_note_reports_the_gate_that_ran() {
        let note = QueueHealer::heal_note("main", &TestGate::Passed("cargo check"));
        assert!(note.contains("Local gate `cargo check` passed"));
        assert!(note.contains("trunk `main`"));
        assert!(!note.contains("Passed local test verification gate"));

        let note = QueueHealer::heal_note("dev", &TestGate::Unavailable);
        assert!(note.contains("not verified"));
    }

    #[test]
    fn package_json_test_script_detection() {
        assert!(QueueHealer::package_json_has_test_script(
            br#"{"scripts":{"test":"vitest run"}}"#
        ));
        assert!(!QueueHealer::package_json_has_test_script(
            br#"{"scripts":{"build":"tsc"}}"#
        ));
        assert!(!QueueHealer::package_json_has_test_script(
            br#"{"scripts":{"test":"   "}}"#
        ));
        assert!(!QueueHealer::package_json_has_test_script(b"not json"));
    }
}
