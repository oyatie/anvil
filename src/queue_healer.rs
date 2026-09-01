use anyhow::{Context, Result, bail};
use regex::Regex;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tracing::{error, info, warn};

pub mod bisector;
pub use bisector::{BisectionResult, MergeTrainBisector};

use crate::exec::ExecClass;
use crate::git_manager::GitManager;
use crate::github::{GitHubClient, PrMetadata};
use crate::merge_enlister::MergeEnlister;
use crate::model_prompt::{HarnessText, ModelPrompt};
use crate::reviewer::untrusted::{Untrusted, UntrustedLabel};

/// Upper bound for one agy repair turn, matching `ExecClass::Model`.
///
/// agy's own `--print-timeout` defaults to 5m0s and fires with
/// `Error: timeout waiting for response` (exit 1) regardless of how long Anvil
/// is willing to wait. The healer therefore passes an explicit `--print-timeout`
/// a little under its own kill so the two deadlines agree and agy's default
/// never silently wins.
const AGY_TURN_LIMIT: Duration = crate::exec::ExecClass::Model.timeout();

/// Builds the exact write-capable queue-repair prompt. Branch names and merge
/// diagnostics remain typed data while their roles and the repair task remain
/// trusted harness text.
pub fn build_queue_repair_prompt(
    repo: &str,
    pr_number: u64,
    base_branch: &str,
    head_branch: &str,
    conflict_details: Option<&str>,
) -> Result<ModelPrompt> {
    let mut prompt = ModelPrompt::builder();
    prompt
        .push_harness(HarnessText::QueuePreamble)
        .push_u64(pr_number)
        .push_harness(HarnessText::QueueRepositoryStart);
    prompt.push_repository(repo)?;
    prompt
        .push_harness(HarnessText::QueueContextAndBaseBranch)
        .push_untrusted(Untrusted::new(UntrustedLabel::BranchName, base_branch))
        .push_harness(HarnessText::QueueHeadBranch)
        .push_untrusted(Untrusted::new(UntrustedLabel::BranchName, head_branch));
    if let Some(details) = conflict_details {
        prompt
            .push_harness(HarnessText::QueueConflictPresent)
            .push_untrusted(Untrusted::new(UntrustedLabel::MergeConflict, details));
    } else {
        prompt.push_harness(HarnessText::QueueNoTextConflict);
    }
    prompt.push_harness(HarnessText::QueueRepairTask);
    prompt.finish()
}

/// Outcome of the local verification gate.
///
/// `Unavailable` is not a pass: a repository without a gate Anvil knows how to
/// run gets no heal pushed, because the heal note would otherwise claim a
/// verification that never happened.
///
/// `Errored` is not a failure. A gate that never completed -- `cargo` or `npm`
/// missing from the daemon's PATH, the `ExecClass::Build` deadline expiring on
/// a cold build in a fresh worktree, the worktree GC reaping the tree mid-build
/// -- measured nothing about the pull request, and the corpus publishes this
/// gate as `test_suite_status` on the pull request and counts it in the
/// approving review. Collapsed into `Failed` it became the sentence "Test suite
/// reported failures during verification gate", which is a specific, checkable
/// accusation that nothing ran; the same code refuses to fabricate the opposite
/// claim twelve lines away. It carries the cause, because "the gate did not
/// complete" is only actionable with the reason it did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestGate {
    Passed(&'static str),
    Failed(&'static str),
    Errored(&'static str, String),
    Unavailable,
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

    /// Comment body for a pushed heal; says only what was actually done.
    ///
    /// `enlistment` is the outcome of the re-enlistment this note used to
    /// announce in the future tense. "*Re-enlisting into GitHub Merge Queue…*"
    /// was written onto the pull request before the certification and the
    /// enlistment it named had run, and both of those now refuse whenever the
    /// corpus cannot measure a gate -- so the sentence was permanent, published,
    /// and in the ordinary configuration false. Derived from the outcome
    /// instead, on the pattern `MergeEnlister::enlistment_note` already sets:
    /// the text says what happened, and the reason when nothing did.
    pub fn heal_note(base_branch: &str, gate: &TestGate, enlistment: &Result<()>) -> String {
        let gate_line = match gate {
            TestGate::Passed(label) => format!("- Local gate `{}` passed", label),
            // Unreachable for a pushed heal; spelled out so the note never lies
            // if the call site changes.
            TestGate::Failed(label) => format!("- Local gate `{}` FAILED", label),
            TestGate::Errored(label, cause) => {
                format!("- Local gate `{}` did not complete: {}", label, cause)
            }
            TestGate::Unavailable => "- No local gate available (not verified)".to_string(),
        };
        let enlistment_line = match enlistment {
            Ok(()) => "*Re-enlisted into the GitHub Merge Queue.*".to_string(),
            Err(e) => format!("*Not re-enlisted into the GitHub Merge Queue:* {:#}", e),
        };
        format!(
            "🛠️ **Merge Queue Self-Healing Applied:**\n\n\
             - Re-synchronized against latest trunk `{}`\n\
             - Merge train divergence repaired by Antigravity\n\
             {}\n\n\
             {}\n\n---\n*🤖 [Healed] by Oyatie Anvil*",
            base_branch, gate_line, enlistment_line
        )
    }

    /// Heals an ejected or failed merge queue PR
    ///
    /// `state` is threaded through so the re-enlistment at the end can run the
    /// certification corpus for the healed head. A local gate is not
    /// certification, and re-enlisting on it was issue #17's fourth door.
    ///
    /// `Ok` carries what happened, because there are three of them and they are
    /// not the same news: the pull request was not open and nothing ran, the
    /// repair produced nothing to push, or a commit was pushed and re-enlisted.
    /// `/api/heal-queue` answers with this string, and "healed and re-enlisted"
    /// asserted over the first two would be the unmeasured claim this whole
    /// change removes -- one layer up from the `let _ =` that used to swallow
    /// the `Err`.
    pub async fn heal_ejected_pr(
        &self,
        state: &crate::webhook::AppState,
        repo: &str,
        pr_number: u64,
    ) -> Result<String> {
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
            return Ok(format!(
                "{}#{} is {}, not OPEN: nothing was healed and nothing was enlisted",
                repo, pr_number, meta.state
            ));
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
        //    Verified, not assumed: step 1 above just moved `FETCH_HEAD` in the
        //    shared clone to the base branch tip, which is exactly what
        //    `create_ephemeral_worktree` falls back to when the head object is
        //    not local. Unchecked, a heal would then be computed, gated, and
        //    force-pushed onto the pull request's branch from a tree checked out
        //    on trunk.
        let worktree = self
            .git_mgr
            .create_ephemeral_worktree(repo, pr_number, &meta.head_ref_oid)
            .await?;
        if let Err(e) = worktree.verify_at(&meta.head_ref_oid).await {
            let _ = worktree.cleanup().await;
            return Err(e).with_context(|| {
                format!(
                    "Queue heal for {}#{} was abandoned before anything was changed",
                    repo, pr_number
                )
            });
        }
        let result = self
            .heal_in_worktree(
                state,
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
        state: &crate::webhook::AppState,
        repo: &str,
        pr_number: u64,
        meta: &PrMetadata,
        base_branch: &str,
        work_dir: &Path,
    ) -> Result<String> {
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
        let prompt = build_queue_repair_prompt(
            repo,
            pr_number,
            base_branch,
            &meta.head_ref_name,
            has_merge_conflict.then_some(conflict_details.as_str()),
        )?;

        self.run_agy_prompt(&prompt, work_dir).await?;

        // 5. Run the local gate; one self-correction turn on failure
        let mut gate = Self::run_local_test_gate(work_dir).await;
        if let TestGate::Failed(label) = gate {
            warn!(
                "Gate `{}` failed after queue healing for {}#{}. Attempting self-correction...",
                label, repo, pr_number
            );
            let mut retry_prompt = ModelPrompt::builder();
            retry_prompt.push_harness(HarnessText::QueueRetryTask);
            let retry_prompt = retry_prompt.finish()?;
            self.run_agy_prompt(&retry_prompt, work_dir).await?;
            gate = Self::run_local_test_gate(work_dir).await;
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
            TestGate::Errored(label, cause) => {
                // Not "still failing": nothing measured this tree. The heal is
                // withheld either way, and the reason an operator is given is
                // the one that is true.
                bail!(
                    "Queue heal for {}#{} not pushed: gate `{}` did not complete, so nothing \
                     verified the repair: {}",
                    repo,
                    pr_number,
                    label,
                    cause
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
        let add_cmd = crate::git_manager::stage_excluding_receipts(work_dir);
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
            return Ok(format!(
                "Queue heal for {}#{} produced no changes to push, so nothing was pushed and \
                 nothing was re-enlisted",
                repo, pr_number
            ));
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

        // 9. Re-enlist, then comment.
        //
        // In that order, because the comment says whether the re-enlistment
        // happened. Posted first it could only announce an intention, and
        // "*Re-enlisting into GitHub Merge Queue...*" is a claim about an
        // action -- written permanently onto the pull request, ahead of a
        // certification that refuses whenever a gate cannot be measured, which
        // in the shipped configuration is always. The note is now posted on
        // every outcome and derived from it, so a reader of the pull request
        // learns that the heal was pushed and the queue was not re-entered,
        // instead of the opposite.
        //
        // The healed head is a different commit from the one any earlier
        // certification judged, so the corpus is run again for it. The local
        // test gate above is not certification and never was.
        //
        // Which commit "it" is comes from this worktree, not from the API.
        // GitHub's view of a PR head is eventually consistent immediately after
        // a push, so re-reading `head_ref_oid` can hand the corpus the pre-heal
        // commit while the merge queue takes the healed one. The healer knows
        // the OID it just pushed; certification is refused unless that is the
        // commit being certified. `evidence_for_enlistment` waits a bounded
        // while for GitHub's view to catch up before it refuses, so the race
        // this comment names is tolerated rather than made fatal by the
        // mitigation for it.
        //
        // Every outcome below is returned rather than logged. Warned and
        // swallowed, a heal that certified nothing and re-enlisted nothing still
        // returned `Ok(())`, so `anvil heal-queue` exited 0 and
        // `POST /api/heal-queue` answered success about a pull request that was
        // never put back in the queue. The push did happen and the error does
        // not undo it; what the error says is that the re-enlistment did not.
        let Some(healed_head) = Self::head_oid(work_dir).await else {
            bail!(
                "Queue heal for {}#{} pushed a commit and then could not read which commit it \
                 was, so nothing was certified and nothing was re-enlisted.",
                repo,
                pr_number
            );
        };
        let enlistment = self
            .certify_and_reenlist(state, repo, pr_number, &healed_head)
            .await;

        let heal_note = Self::heal_note(base_branch, &gate, &enlistment);
        if let Err(e) = self
            .github_client
            .post_pr_comment(repo, pr_number, &heal_note)
            .await
        {
            warn!("Could not post heal note on {}#{}: {}", repo, pr_number, e);
        }

        enlistment.map(|()| {
            format!(
                "Queue heal for {}#{} pushed {} and re-enlisted it in the merge queue",
                repo, pr_number, healed_head
            )
        })
    }

    /// Certifies the commit the heal just pushed and hands it back to the merge
    /// queue.
    ///
    /// Split out so `heal_in_worktree` can hold the outcome as a value: the
    /// heal note is derived from it and the caller is answered with it, and a
    /// `?` in the middle of the push-comment-enlist sequence could do neither.
    ///
    /// `pub` for the reason `MergeEnlister::subject_refusal` is: this is the
    /// re-enlist door, an integration test sees only `pub` items, and the only
    /// public way in is `heal_ejected_pr`, which clones, writes and pushes
    /// before it gets here. Left private the door was pinned by a source scan
    /// and nothing else.
    pub async fn certify_and_reenlist(
        &self,
        state: &crate::webhook::AppState,
        repo: &str,
        pr_number: u64,
        healed_head: &str,
    ) -> Result<()> {
        let evidence = crate::webhook::pipelines::certify::evidence_for_enlistment(
            state,
            repo,
            pr_number,
            Some(healed_head),
        )
        .await
        .with_context(|| {
            format!(
                "Queue heal for {}#{} pushed {} and nothing was re-enlisted: no certification \
                 could be obtained for it",
                repo, pr_number, healed_head
            )
        })?;
        self.merge_enlister
            .enlist_into_merge_queue(repo, pr_number, Some(&evidence))
            .await
            .with_context(|| {
                format!(
                    "Queue heal for {}#{} pushed {} and it was not re-enlisted",
                    repo, pr_number, healed_head
                )
            })
    }

    /// The commit at `HEAD` in a working tree, or `None` when git could not say.
    ///
    /// `None` withholds: a heal that cannot name the commit it pushed has no
    /// commit to certify.
    async fn head_oid(work_dir: &Path) -> Option<String> {
        let mut cmd = Command::new("git");
        cmd.current_dir(work_dir).args(["rev-parse", "HEAD"]);
        let out = crate::exec::run_bounded(
            cmd,
            crate::exec::ExecClass::Quick,
            "git rev-parse HEAD (queue healer)",
        )
        .await
        .ok()?;
        if !out.status.success() {
            return None;
        }
        let sha = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (!sha.is_empty()).then_some(sha)
    }

    /// Picks the gate from what the repository provides and runs it.
    ///
    /// Three distinct answers, because the corpus publishes this one onto a
    /// pull request: the gate ran and passed, the gate ran and reported
    /// failures, or the gate never completed. Only the second is a statement
    /// about the pull request.
    ///
    /// An associated function rather than a method: the certification corpus
    /// reports the same gate as `test_suite_status`, and the alternative to
    /// sharing it was the literal `Some(true)` the review pipeline used to pass
    /// for a suite it never ran.
    ///
    /// # What it runs, and why the Cargo arm takes two steps
    ///
    /// For a Cargo repository this ran `cargo check`, which type-checks the
    /// crate, builds no test binary and executes no test. Publishing that as
    /// `test_suite_status` meant the gate named "Automated Test Suite" reported
    /// a pass on every Rust pull request whose code merely compiled, including
    /// trees in which every test was red — and reported "Test suite reported
    /// failures during verification gate" for a tree that did not compile,
    /// which is a failure of a different suite than the one named.
    ///
    /// It now runs the suite. In two invocations, because `cargo`'s documented
    /// exit statuses are `0` and `101` only, and libtest also exits `101` when
    /// tests fail: one `cargo test` cannot tell "did not compile" from "tests
    /// failed", and those are `Errored` and `Failed` here — absent evidence
    /// versus an accusation published on a contributor's pull request. Building
    /// as its own step makes the distinction unambiguous with nothing installed
    /// beyond `cargo`. `cargo nextest` reports the two as exit 101 and 100 in a
    /// single run and would be strictly better, but it is a separate install
    /// this gate cannot assume on a daemon host, and it skips doctests.
    ///
    /// `--no-fail-fast` matches this repository's own CI SSOT
    /// (`.config/nextest.toml` sets `fail-fast = false`): a gate that stops at
    /// the first red binary reports one failure where there are twenty.
    ///
    /// # Cost
    ///
    /// The tree is an ephemeral worktree with no `target/`, so both steps are
    /// cold. Measured on this repository (18 cores, ~1000 tests, a fresh target
    /// directory and a warm registry): `cargo check` 15.7s, `cargo test
    /// --no-run` 24.6s, the run after it 37.0s.
    ///
    /// The multiplier is the part that carries to another repository; the
    /// seconds are this host's. Building every test binary costs **1.6×** a
    /// type-check of the library, and the whole gate costs **~3.9×** the
    /// command it replaced. A reader sizing this for a monorepo should scale
    /// that ratio, not add a fixed delta.
    ///
    /// The whole gate shares one `ExecClass::Build` deadline rather than taking
    /// one per step, so its worst case is the class bound and not twice it.
    /// That bound is 1800s and was sized for a type-check; `heal_ejected_pr`
    /// calls this gate twice, so one heal can spend an hour. Hitting it is
    /// `Errored` → `NotMeasured` → merge withheld with no accusation, which is
    /// the right failure, but it is a fleet-wide stall rather than a rounding
    /// error.
    ///
    /// # Known ceilings
    ///
    /// A Cargo repository with no tests at all exits `0` and is reported as a
    /// pass. `cargo` has no distinct signal for it; nextest's `NO_TESTS_RUN =
    /// 4` does, and would be the way to close it.
    ///
    /// `cargo test --no-run` does not build doctests. A doctest that fails to
    /// *compile* therefore survives the build step and reaches the run, where
    /// it is classified `Failed` — the one compile error this gate still
    /// reports as a failing suite. Defensible, because rustdoc reports it as a
    /// failed test, but it is the exception to the split above. Doctests cost
    /// 1.0s of the 37.0s run here.
    ///
    /// The run executes every `#[test]` in a contributor's branch inside the
    /// daemon's process environment, which holds `GITHUB_WEBHOOK_SECRET`. A
    /// type-check never ran that code. The child environment is not otherwise
    /// scrubbed.
    pub async fn run_local_test_gate(repo_dir: &Path) -> TestGate {
        if repo_dir.join("Cargo.toml").exists() {
            return Self::run_cargo_test_gate(repo_dir).await;
        }
        if !Self::has_npm_test_script(repo_dir).await {
            return TestGate::Unavailable;
        }

        let label = "npm test";
        let mut cmd = crate::exec::build_env::command("npm");
        cmd.args(["test", "--silent"]).current_dir(repo_dir);
        Self::classify(
            label,
            crate::exec::run_bounded(cmd, ExecClass::Build, label).await,
        )
    }

    /// Build, then run. See `run_local_test_gate` for why the two are separate
    /// invocations and why they share one deadline.
    ///
    /// Both spawns drop `CARGO_TARGET_DIR` and `CARGO_BUILD_TARGET_DIR` from
    /// the inherited environment. The two steps only discriminate a compile
    /// error from a test failure because the second finds the artefacts the
    /// first built; a target directory shared with anything else breaks that.
    /// Every ephemeral worktree of a repository carries the same package name
    /// and version, so concurrent certifications of the same repository resolve
    /// to the same artefact path, and the observed result is the pre-fix
    /// behaviour exactly: `Passed` on a red suite, `Failed` on a tree that does
    /// not compile. `tests/test_suite_gate_shared_target_test.rs` runs the gate
    /// with that variable set and pins all three answers.
    async fn run_cargo_test_gate(repo_dir: &Path) -> TestGate {
        // Two labels, because a reader of `Errored("cargo test", ...)` on a
        // tree that did not build is told a command failed that was never the
        // one that failed.
        const BUILD_LABEL: &str = "cargo test --no-run";
        let label = "cargo test";
        let deadline = Instant::now() + ExecClass::Build.timeout();

        let mut build = crate::exec::build_env::command("cargo");
        build.args(["test", "--no-run"]).current_dir(repo_dir);
        match crate::exec::run_bounded(build, ExecClass::Build, BUILD_LABEL).await {
            Ok(out) if out.status.success() => {}
            // Not `Failed`. A tree that does not build ran no test, so it is a
            // gate that did not complete, which `local_verification_gate` maps
            // to `None` and the corpus to `NotMeasured` — withholding the merge
            // without accusing the pull request of a failing suite.
            Ok(out) => {
                let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
                warn!(
                    "the test suite could not be built in the local gate: {}",
                    why
                );
                return TestGate::Errored(
                    BUILD_LABEL,
                    format!("the test suite did not build: {why}"),
                );
            }
            Err(e) => {
                warn!(
                    "cargo test --no-run did not complete in the local gate: {:#}",
                    e
                );
                return TestGate::Errored(BUILD_LABEL, format!("{e:#}"));
            }
        }

        // The remainder of the one deadline the whole gate gets. Zero when the
        // build consumed it, which `run_bounded_for` reports as a timeout —
        // correctly, because no test ran.
        let remaining = deadline.saturating_duration_since(Instant::now());
        let mut run = crate::exec::build_env::command("cargo");
        run.args(["test", "--no-fail-fast"]).current_dir(repo_dir);
        Self::classify(
            label,
            crate::exec::run_bounded_for(run, remaining, label).await,
        )
    }

    /// The three answers a completed, failed or absent run maps to. Shared by
    /// both arms so neither can drift into calling absent evidence a failure.
    fn classify(label: &'static str, outcome: Result<std::process::Output>) -> TestGate {
        match outcome {
            Ok(out) if out.status.success() => TestGate::Passed(label),
            Ok(out) => {
                warn!(
                    "{} reported failures in the local gate: {}",
                    label,
                    String::from_utf8_lossy(&out.stderr).trim()
                );
                TestGate::Failed(label)
            }
            Err(e) => {
                warn!("{} did not complete in the local gate: {:#}", label, e);
                TestGate::Errored(label, format!("{e:#}"))
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

    async fn run_agy_prompt(&self, prompt: &ModelPrompt, working_dir: &Path) -> Result<String> {
        let cmd = crate::exec::agy_agent(
            &crate::exec::Posture::in_workspace(working_dir),
            &self.agy_effort,
            AGY_TURN_LIMIT,
            None,
        )?;

        let turn = crate::exec::turn::run(cmd, prompt, AGY_TURN_LIMIT, "agy (queue healer)")
            .await
            .context("Failed to run agy command")?;

        if !turn.status.success() {
            error!(
                "agy returned non-zero status in QueueHealer: {}",
                turn.status
            );
            warn!("agy stderr: {}", turn.stderr.trim());
        }
        turn.into_result()
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
        let r = crate::exec::interpret_agy_outcome(
            false,
            "Inspecting the workspace...\n",
            "Error: timeout waiting for response\n",
        );
        let err = r.expect_err("non-zero agy exit must not be a repair");
        assert!(err.to_string().contains("timeout waiting for response"));

        let ok = crate::exec::interpret_agy_outcome(true, "done", "").unwrap();
        assert_eq!(ok, "done");
    }

    #[test]
    fn healer_turn_limit_matches_model_class() {
        assert_eq!(AGY_TURN_LIMIT, crate::exec::ExecClass::Model.timeout());
        assert_eq!(crate::exec::agy_print_timeout_arg(AGY_TURN_LIMIT), "570s");
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
    fn heal_note_reports_the_gate_that_ran() {
        let note = QueueHealer::heal_note("main", &TestGate::Passed("cargo test"), &Ok(()));
        assert!(note.contains("Local gate `cargo test` passed"));
        assert!(note.contains("trunk `main`"));
        assert!(!note.contains("Passed local test verification gate"));

        let note = QueueHealer::heal_note("dev", &TestGate::Unavailable, &Ok(()));
        assert!(note.contains("not verified"));
    }

    /// The note reports the re-enlistment that happened, not the one that was
    /// about to be attempted.
    #[test]
    fn heal_note_reports_the_re_enlistment_outcome() {
        let enlisted = QueueHealer::heal_note("main", &TestGate::Passed("cargo test"), &Ok(()));
        let withheld = QueueHealer::heal_note(
            "main",
            &TestGate::Passed("cargo test"),
            &Err(anyhow::anyhow!("slo_status produced no measurement")),
        );
        assert_ne!(
            enlisted, withheld,
            "the same note was published for a heal that was re-enlisted and one that was not"
        );
        assert!(!enlisted.contains("Re-enlisting"));
        assert!(!withheld.contains("Re-enlisting"));
        assert!(withheld.contains("Not re-enlisted"));
        assert!(withheld.contains("slo_status produced no measurement"));
    }

    /// A gate that never completed is not a gate that reported failures.
    #[test]
    fn heal_note_separates_a_gate_that_did_not_complete_from_one_that_failed() {
        let failed = QueueHealer::heal_note("main", &TestGate::Failed("cargo test"), &Ok(()));
        let errored = QueueHealer::heal_note(
            "main",
            &TestGate::Errored(
                "cargo test",
                "No such file or directory (os error 2)".into(),
            ),
            &Ok(()),
        );
        assert!(failed.contains("FAILED"));
        assert!(!errored.contains("FAILED"));
        assert!(errored.contains("did not complete"));
        assert!(errored.contains("No such file or directory"));
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
