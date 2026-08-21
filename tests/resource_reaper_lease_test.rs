//! Contract for the leased-worktree reaper. RED by design: the lease API these
//! tests drive does not exist yet.
//!
//! Anvil's reaper is wired to a 10s cadence (`src/self_governance/mod.rs:64`)
//! and reclaims nothing: the call passes `None`, and `run_sweep`'s only
//! worktree code sits behind `if let Some(dir) = repo_dir`. Its one mechanism,
//! `git worktree prune`, removes only entries whose directory is already gone.
//! Measured consequence: 247 worktrees on oyatie, 8 on console, 34 resident
//! buck2 daemons holding 7.6 GB RSS while idle.
//!
//! The required design is that worktrees are LEASED, not merely created:
//! a lease records the owner PID and a creation time, a worktree becomes
//! reclaimable only when the owner process is dead OR the lease has expired,
//! and reclaim goes through `git worktree remove` so git's own dirty-tree
//! refusal is in the loop.
//!
//! The safety requirement is not incidental. A live audit found 80 of 159
//! stale worktrees holding 4,332 tracked-file modifications that existed
//! nowhere else. `git worktree remove --force` would have destroyed all of it.
//! A dirty worktree must be refused and reported, never force-removed.
//!
//! API these tests require (to be built in the implementation stage):
//!   - `anvil::self_governance::worktree_lease::{WorktreeLease, LeaseStore}`
//!   - `WorktreeLease` with public `repo_dir`, `worktree_path`, `owner_pid`,
//!     `created_at: SystemTime`, `ttl: Duration` so a test can backdate a lease
//!     without sleeping.
//!   - `LeaseStore::new(root)`, `record(&lease).await`, persisting in a format
//!     the reaper reads back. The on-disk shape is deliberately not asserted.
//!   - `AutonomousResourceReaper::with_lease_store(staging_dirs, store)`
//!   - `run_sweep(&self)` taking NO repo argument, so a dead `None` branch is
//!     structurally impossible to reintroduce.
//!   - `GarbageCollectionReport` gaining `worktrees_inspected: usize`,
//!     `worktrees_reclaimed: Vec<PathBuf>`, `worktrees_refused_dirty: Vec<PathBuf>`.
//!
//! Per the task's standing constraint, nothing here arms a sweep against a real
//! repository: every fixture is a throwaway tempfile-backed git repo, and the
//! reaper is always built with an EMPTY staging dir list so it can never touch
//! $TMPDIR/anvil* on a developer's machine.

use anvil::self_governance::AutonomousResourceReaper;
use anvil::self_governance::worktree_lease::{LeaseStore, WorktreeLease};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, SystemTime};

fn git(dir: &Path, args: &[&str]) -> Output {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "anvil-test")
        .env("GIT_AUTHOR_EMAIL", "anvil-test@example.invalid")
        .env("GIT_COMMITTER_NAME", "anvil-test")
        .env("GIT_COMMITTER_EMAIL", "anvil-test@example.invalid")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to exec git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {:?} in {} failed: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// A real repo plus one real worktree whose directory is present on disk --
/// the shape every one of the 247 audited worktrees had.
fn repo_with_worktree(root: &Path, branch: &str) -> (PathBuf, PathBuf) {
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).expect("create repo dir");
    git(&repo, &["init", "--quiet"]);
    std::fs::write(repo.join("tracked.txt"), "original\n").expect("seed file");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "--quiet", "-m", "seed"]);

    let wt = root.join("worktrees").join(branch);
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            branch,
            wt.to_str().expect("utf8 worktree path"),
        ],
    );
    assert!(wt.is_dir(), "fixture invalid: worktree dir not created");
    (repo, wt)
}

/// A PID that has provably exited: spawned, reaped, and never re-parented.
fn dead_pid() -> u32 {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg("exit 0")
        .spawn()
        .expect("spawn throwaway process");
    let pid = child.id();
    child.wait().expect("reap throwaway process");
    pid
}

fn expired_lease(repo: &Path, wt: &Path, owner_pid: u32) -> WorktreeLease {
    WorktreeLease {
        repo_dir: repo.to_path_buf(),
        worktree_path: wt.to_path_buf(),
        owner_pid,
        created_at: SystemTime::now() - Duration::from_secs(7200),
        ttl: Duration::from_secs(3600),
    }
}

fn live_lease(repo: &Path, wt: &Path, owner_pid: u32) -> WorktreeLease {
    WorktreeLease {
        repo_dir: repo.to_path_buf(),
        worktree_path: wt.to_path_buf(),
        owner_pid,
        created_at: SystemTime::now(),
        ttl: Duration::from_secs(3600),
    }
}

fn reaper_for(store_root: &Path) -> AutonomousResourceReaper {
    // Empty staging dirs: this must never be able to reach $TMPDIR/anvil*.
    AutonomousResourceReaper::with_lease_store(vec![], LeaseStore::new(store_root))
}

// ---------------------------------------------------------------------------
// (1) The dead `None` branch is gone.
// ---------------------------------------------------------------------------

/// DEFECT CAUGHT: `run_sweep(None)` from `src/self_governance/mod.rs:64` skips
/// the entire worktree path, because that path lives inside
/// `if let Some(dir) = repo_dir`. ~8,640 sweeps a day inspect zero worktrees.
///
/// This test drives the sweep the way production does -- with no repo argument
/// at all -- and requires that it still inspects the leases it owns. Once
/// `run_sweep` takes no `Option<&Path>`, the dead branch cannot be written.
///
/// Why prompting would not prevent it: an instruction like "make sure the
/// reaper actually runs" is satisfied by this code. It runs, on schedule,
/// returns `Ok`, and increments nothing. The failure is a silent no-op inside
/// a healthy-looking loop; only an assertion that the sweep observed a real
/// worktree distinguishes running from working.
#[tokio::test]
async fn production_shaped_sweep_inspects_leased_worktrees() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (repo, wt) = repo_with_worktree(tmp.path(), "stale-1");
    let store_root = tmp.path().join("leases");

    let store = LeaseStore::new(&store_root);
    store
        .record(&expired_lease(&repo, &wt, dead_pid()))
        .await
        .expect("record lease");

    let report = reaper_for(&store_root)
        .run_sweep()
        .await
        .expect("sweep should not error");

    assert!(
        report.worktrees_inspected >= 1,
        "sweep invoked with no repo argument inspected {} worktrees despite a \
         recorded lease at {}; the None dead branch is still present",
        report.worktrees_inspected,
        wt.display()
    );
}

// ---------------------------------------------------------------------------
// (2) A live owner is never reclaimed.
// ---------------------------------------------------------------------------

/// DEFECT CAUGHT: age-based reaping with no ownership check. The shipped sweep
/// deletes staging entries purely on mtime > 1h
/// (`resource_reaper.rs`, the `age > Duration::from_secs(3600)` arm), with no
/// notion of who owns the directory. Extending that policy to worktrees would
/// delete the working tree out from under a running agent.
///
/// The lease's owner PID is the guard: this test uses `std::process::id()`,
/// which is alive by construction for as long as the test runs.
///
/// Why prompting would not prevent it: "reap stale worktrees" is a schedule,
/// not a predicate. Under load an agent legitimately holds a worktree longer
/// than any wall-clock threshold you pick, and the resulting deletion looks
/// like a flaky build rather than a reaper bug. Liveness has to be asserted.
#[tokio::test]
async fn worktree_owned_by_a_live_process_is_not_reclaimed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (repo, wt) = repo_with_worktree(tmp.path(), "owned-by-live");
    let store_root = tmp.path().join("leases");

    let store = LeaseStore::new(&store_root);
    // Expired lease, but the owner is THIS test process -- unambiguously alive.
    store
        .record(&expired_lease(&repo, &wt, std::process::id()))
        .await
        .expect("record lease");

    let report = reaper_for(&store_root)
        .run_sweep()
        .await
        .expect("sweep should not error");

    assert!(
        wt.is_dir(),
        "reaper deleted worktree {} whose owner PID {} is alive",
        wt.display(),
        std::process::id()
    );
    assert!(
        !report.worktrees_reclaimed.contains(&wt),
        "reaper reported reclaiming a live-owned worktree: {:?}",
        report.worktrees_reclaimed
    );
}

// ---------------------------------------------------------------------------
// (3) Dirty worktrees are never force-removed. THE SAFETY TEST.
// ---------------------------------------------------------------------------

/// DEFECT CAUGHT: destroying uncommitted work. A live audit found 80 of 159
/// stale worktrees carrying 4,332 tracked-file modifications that existed in no
/// commit, no stash and no remote. `git worktree remove --force` -- the obvious
/// way to make reclaim "just work" when plain `remove` starts refusing things --
/// deletes every one of them irrecoverably.
///
/// This test hands the reaper the maximum pressure to remove: the lease is
/// expired AND the owner PID is dead. It must still refuse, leave the file
/// byte-identical, and name the worktree in `worktrees_refused_dirty` so the
/// operator learns about it.
///
/// Why prompting would not prevent it: the failure is silent and delayed. A
/// reaper that force-removes passes every test about reclaiming, frees the most
/// disk, and produces the cleanest logs; the damage surfaces days later as work
/// someone cannot find, with no error to trace back. And `--force` is exactly
/// what a developer reaches for when `git worktree remove` returns non-zero in
/// CI. Only an assertion on preserved content holds the line.
#[tokio::test]
async fn dirty_worktree_is_never_force_removed_even_with_an_expired_lease() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (repo, wt) = repo_with_worktree(tmp.path(), "dirty-work");
    let store_root = tmp.path().join("leases");

    // Uncommitted modification to a TRACKED file: exists in no commit anywhere.
    let precious = wt.join("tracked.txt");
    std::fs::write(&precious, "UNCOMMITTED WORK THAT EXISTS NOWHERE ELSE\n")
        .expect("dirty the worktree");
    let status = git(&wt, &["status", "--porcelain"]);
    assert!(
        !status.stdout.is_empty(),
        "fixture invalid: worktree is not actually dirty"
    );

    let store = LeaseStore::new(&store_root);
    store
        .record(&expired_lease(&repo, &wt, dead_pid()))
        .await
        .expect("record lease");

    let report = reaper_for(&store_root)
        .run_sweep()
        .await
        .expect("sweep should not error");

    assert!(
        wt.is_dir(),
        "SAFETY VIOLATION: reaper removed dirty worktree {}",
        wt.display()
    );
    assert_eq!(
        std::fs::read_to_string(&precious).expect("read preserved file"),
        "UNCOMMITTED WORK THAT EXISTS NOWHERE ELSE\n",
        "SAFETY VIOLATION: reaper destroyed uncommitted tracked-file changes"
    );
    assert!(
        !report.worktrees_reclaimed.contains(&wt),
        "dirty worktree was counted as reclaimed: {:?}",
        report.worktrees_reclaimed
    );
    assert!(
        report.worktrees_refused_dirty.contains(&wt),
        "reaper refused the dirty worktree silently; it must be reported so an \
         operator can rescue the work. refused list was {:?}",
        report.worktrees_refused_dirty
    );
}

// ---------------------------------------------------------------------------
// (4) An expired lease over a clean tree IS reclaimable.
// ---------------------------------------------------------------------------

/// DEFECT CAUGHT: the reaper reclaiming nothing at all -- the headline defect.
/// This is the positive half of the contract, and it is what makes tests (2)
/// and (3) meaningful: without it, "never remove anything" would pass them
/// both, which is precisely the behaviour shipping today.
///
/// Why prompting would not prevent it: the shipped code was written to do this
/// and does not. Intent was never the missing ingredient; an executable
/// assertion against a real worktree is.
#[tokio::test]
async fn expired_lease_over_a_clean_tree_is_reclaimed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (repo, wt) = repo_with_worktree(tmp.path(), "expired-clean");
    let store_root = tmp.path().join("leases");

    let status = git(&wt, &["status", "--porcelain"]);
    assert!(
        status.stdout.is_empty(),
        "fixture invalid: worktree should be clean, got {}",
        String::from_utf8_lossy(&status.stdout)
    );

    let store = LeaseStore::new(&store_root);
    store
        .record(&expired_lease(&repo, &wt, dead_pid()))
        .await
        .expect("record lease");

    let report = reaper_for(&store_root)
        .run_sweep()
        .await
        .expect("sweep should not error");

    assert!(
        !wt.exists(),
        "expired lease over a clean worktree was not reclaimed: {} still on disk",
        wt.display()
    );
    assert!(
        report.worktrees_reclaimed.contains(&wt),
        "reclaim was not reported: {:?}",
        report.worktrees_reclaimed
    );
}

// ---------------------------------------------------------------------------
// (5) Reclaim does not lean on `git worktree prune`.
// ---------------------------------------------------------------------------

/// DEFECT CAUGHT: `git worktree prune` is the reaper's ONLY worktree mechanism,
/// and prune by definition drops only administrative entries whose working
/// directory is already missing. In the live audit all 247 oyatie worktrees had
/// directories present, so prune would have removed approximately zero.
///
/// This test pins the distinguishing case: the directory is verified present
/// immediately before the sweep, so a prune-only implementation provably cannot
/// pass. Afterwards both the directory and git's own administrative entry must
/// be gone -- the pair that only `git worktree remove` produces.
///
/// A second worktree with a live lease is present to prove reclaim is
/// per-lease and not a blanket "prune everything" that happens to clear the
/// target.
///
/// Why prompting would not prevent it: this is a gitworktree(1) semantics trap,
/// not a lapse in care. "prune" names the general concept of garbage
/// collection, git spells it as a narrow special case, and the code even checks
/// prune's exit status and counts a success -- so it reports work for a no-op.
/// Reading the source cannot tell you which meaning git implements; running it
/// against a worktree that still has a directory can.
#[tokio::test]
async fn reclaim_removes_a_worktree_whose_directory_still_exists() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (repo, target) = repo_with_worktree(tmp.path(), "prune-immune");

    // A second, actively-leased worktree that must survive.
    let keeper = tmp.path().join("worktrees").join("keeper");
    git(
        &repo,
        &[
            "worktree",
            "add",
            "--quiet",
            "-b",
            "keeper",
            keeper.to_str().expect("utf8"),
        ],
    );

    let store_root = tmp.path().join("leases");
    let store = LeaseStore::new(&store_root);
    store
        .record(&expired_lease(&repo, &target, dead_pid()))
        .await
        .expect("record expired lease");
    store
        .record(&live_lease(&repo, &keeper, std::process::id()))
        .await
        .expect("record live lease");

    // The condition prune cannot handle: the directory is THERE.
    assert!(
        target.is_dir(),
        "fixture invalid: target directory must exist for this test to \
         distinguish `worktree remove` from `worktree prune`"
    );

    reaper_for(&store_root)
        .run_sweep()
        .await
        .expect("sweep should not error");

    let listed = String::from_utf8_lossy(&git(&repo, &["worktree", "list"]).stdout).to_string();

    assert!(
        !target.exists(),
        "worktree directory {} survived the sweep -- consistent with \
         `git worktree prune`, which only handles already-deleted directories",
        target.display()
    );
    assert!(
        !listed.contains("prune-immune"),
        "git still lists the reclaimed worktree; reclaim must go through \
         `git worktree remove`.\ngit worktree list:\n{listed}"
    );
    assert!(
        keeper.is_dir() && listed.contains("keeper"),
        "reclaim was not per-lease: the live-leased worktree was also removed.\n\
         git worktree list:\n{listed}"
    );
}

/// `--force` must never reach `git worktree remove`.
///
/// The reaper refuses a dirty worktree via its own `git status --porcelain`
/// probe, and that probe short-circuits before the removal command is built.
/// That makes the status check the ONLY defence actually exercised: injecting
/// `--force` into the removal path passes every behavioural test in this file,
/// because no test reaches the command with a dirty tree.
///
/// Plain `git worktree remove` exits 128 on a dirty tree with
/// `fatal: '...' contains modified or untracked files, use --force to delete it`,
/// leaving the bytes untouched. That refusal is the second line of defence the
/// module documents, and it exists only while `--force` is absent.
///
/// A live audit found 80 of 159 stale worktrees holding 4,332 tracked-file
/// modifications that existed nowhere else. `--force` would have destroyed all
/// of it. Prompting will not keep that flag out of the code; this will.
#[test]
fn force_is_never_passed_to_git_worktree_remove() {
    let src = std::fs::read_to_string("src/self_governance/resource_reaper.rs")
        .expect("resource_reaper.rs must exist");

    // Comments explain why --force is banned; they must not trip the ban.
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !code.contains("--force"),
        "`--force` appears in the reaper's removal path. Plain `git worktree remove` \
         refuses a dirty tree and leaves it byte-identical; --force deletes it. The \
         status probe short-circuits before this command, so no behavioural test in \
         this file would catch the flag being added."
    );
}
