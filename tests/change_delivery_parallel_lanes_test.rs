//! N disjoint shards are N lanes, not one lane run N times.
//!
//! `select_independent` computes a set no two of whose members share a touched
//! path or an owner, and `create_lane` gives each member its own worktree. The
//! executor is what decides whether that guarantee buys anything: run the set
//! one at a time and it buys nothing, at the price of one `cargo check` per
//! shard in series.
//!
//! Proving concurrency by timing is a flake. These tests prove it structurally
//! instead: every lane's gate waits on a `Barrier` sized to the number of
//! lanes, so the barrier can only release if all of them are in flight at once.
//! A folded executor never assembles the barrier and the test times out. The
//! timeout is a deadlock detector, not a stopwatch.
//!
//! The lanes are real `git worktree add --detach` worktrees produced by the
//! production adapter, so "isolated" here is git's isolation and not a fixture
//! that mimics it.

use anvil::change_delivery::adapters::GitLaneVcs;
use anvil::change_delivery::adapters::rewrite_mechanical::MechanicalRewrite;
use anvil::change_delivery::facade::deliver::build_lanes;
use anvil::change_delivery::ports::{
    GateResult, LandingPolicy, LaneWorktree, LocalGate, MOVE_PLAN_SCHEMA_V1, Move, MoveKind,
    OwnerMap, ShapeMovePlan, Shard, VcsPort, select_independent, shard_plan,
};
use async_trait::async_trait;
use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Barrier;

const N: usize = 4;

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@x")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@x")
        // Hermetic: the fixture must not inherit whatever the developer has
        // configured, or the proof is about their machine.
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args(args)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A gate that reports what was in flight beside it.
///
/// `live` is incremented before the barrier and decremented after, so `peak`
/// is the number of lanes genuinely overlapping and not a count of lanes that
/// merely happened.
struct BarrierGate {
    barrier: Arc<Barrier>,
    live: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}

#[async_trait]
impl LocalGate for BarrierGate {
    async fn run(&self, _lane: &LaneWorktree) -> GateResult {
        let now = self.live.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        self.barrier.wait().await;
        self.live.fetch_sub(1, Ordering::SeqCst);
        GateResult::Passed {
            label: "barrier".into(),
        }
    }
}

/// One repository with `N` units, each holding one aliased satellite file, so
/// the plan yields `N` shards with pairwise-disjoint touch sets and owners.
fn fixture(dir: &Path) {
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "commit.gpgsign", "false"]);
    for i in 0..N {
        std::fs::create_dir_all(dir.join(format!("unit{i}/policies"))).unwrap();
        std::fs::write(dir.join(format!("unit{i}/manifest.json")), "{}").unwrap();
        std::fs::write(dir.join(format!("unit{i}/policies/p.json")), "{}").unwrap();
    }
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "fixture"]);
}

fn plan_over_units() -> (ShapeMovePlan, OwnerMap) {
    let moves = (0..N)
        .map(|i| Move {
            kind: MoveKind::MoveFile,
            from: format!("unit{i}/policies/p.json"),
            to: format!("unit{i}/policy/p.json"),
            unit: format!("unit{i}"),
            rule_id: "satellite_alias_used".into(),
            evidence: String::new(),
            anchor: None,
            destination_stable: true,
            rank: 20,
        })
        .collect();
    let codeowners = (0..N)
        .map(|i| format!("unit{i}/ @team-{i}\n"))
        .collect::<String>();
    (
        ShapeMovePlan {
            schema: MOVE_PLAN_SCHEMA_V1.into(),
            repo: "example/monorepo".into(),
            rev: "a".repeat(40),
            spec_version: "v1".into(),
            moves,
        },
        OwnerMap::from_codeowners(&codeowners),
    )
}

/// The policy the daemon ships caps concurrency at `max_open_shape_prs` (2).
/// This raises it so the executor, not the cap, is what the test is about.
fn wide_policy() -> LandingPolicy {
    LandingPolicy {
        max_open_shape_prs: N as u32,
        ..LandingPolicy::default()
    }
}

fn head_sha(dir: &Path) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .expect("rev-parse");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn n_disjoint_shards_are_built_in_n_concurrent_lanes() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    fixture(&repo);
    let sha = head_sha(&repo);

    let (plan, owners) = plan_over_units();
    let policy = wide_policy();
    let shards = shard_plan(&plan, &owners, &[], &policy);
    let selected: Vec<Shard> = select_independent(&shards, &[], &policy);
    assert_eq!(
        selected.len(),
        N,
        "the fixture must offer {N} independent shards, else this proves nothing about folding"
    );

    let vcs = GitLaneVcs::new(repo.join(".anvil-lanes"));
    let mut bound = Vec::new();
    for shard in selected {
        let id = format!("p{}", shard.unit);
        let lane = vcs.create_lane(&repo, &id, &sha, true).await;
        assert!(lane.is_ok(), "lane must bind: {lane:?}");
        bound.push((shard, lane));
    }

    // Each lane is a linked worktree, not a checkout of the shared tree: in a
    // linked worktree `.git` is a file pointing at the common dir. A folded
    // executor that reused one tree could not satisfy the barrier below, and
    // a fixture that faked lanes with `git checkout` could not satisfy this.
    for (_, lane) in &bound {
        let dot_git = lane.as_ref().unwrap().path.join(".git");
        assert!(
            dot_git.is_file(),
            "{} is not a linked worktree; one lane, one worktree",
            dot_git.display()
        );
    }
    let paths: BTreeSet<_> = bound
        .iter()
        .map(|(_, l)| l.as_ref().unwrap().path.clone())
        .collect();
    assert_eq!(paths.len(), N, "{N} lanes must be {N} distinct worktrees");

    let peak = Arc::new(AtomicUsize::new(0));
    let gate = BarrierGate {
        barrier: Arc::new(Barrier::new(N)),
        live: Arc::new(AtomicUsize::new(0)),
        peak: Arc::clone(&peak),
    };

    // The deadlock detector. A barrier of N releases only when N lanes are
    // inside it at once; an executor that runs lanes one at a time parks the
    // first one forever and this elapses.
    let runs = tokio::time::timeout(
        Duration::from_secs(30),
        build_lanes(&vcs, &MechanicalRewrite, &gate, &sha, bound),
    )
    .await
    .expect(
        "lanes were not all in flight together: a barrier of N never assembled, \
         which is what folding N ready lanes onto fewer workers looks like",
    );

    assert_eq!(
        peak.load(Ordering::SeqCst),
        N,
        "peak in-flight lanes must be {N}; anything less is a fold"
    );
    assert_eq!(runs.len(), N, "one run per shard");
    for r in &runs {
        assert!(r.rewrite.is_ok(), "rewrite refused: {:?}", r.rewrite);
        assert!(
            matches!(r.gate, Some(GateResult::Passed { .. })),
            "gate result must reach the run: {:?}",
            r.gate
        );
    }

    // The seed tree is untouched: the work happened in the lanes.
    assert_eq!(
        head_sha(&repo),
        sha,
        "seed HEAD moved during the parallel phase"
    );
    assert!(
        repo.join("unit0/policies/p.json").exists(),
        "a lane rewrote the seed working tree; lanes are not isolated"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_lane_that_could_not_be_bound_still_reports_why() {
    // A shard whose lane failed must appear in the report saying so. Dropping
    // it would let a refused shard read as a shard that was never planned.
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    fixture(&repo);
    let sha = head_sha(&repo);

    let (plan, owners) = plan_over_units();
    let policy = wide_policy();
    let shards = shard_plan(&plan, &owners, &[], &policy);
    let mut selected = select_independent(&shards, &[], &policy);
    selected.truncate(2);

    let vcs = GitLaneVcs::new(repo.join(".anvil-lanes"));
    // A short sha is refused by the adapter: a lane base must be a full sha.
    let good = vcs.create_lane(&repo, "ok", &sha, true).await;
    let bad = vcs.create_lane(&repo, "bad", "abc123", true).await;
    assert!(bad.is_err(), "a short base sha must be refused");

    let gate = BarrierGate {
        barrier: Arc::new(Barrier::new(1)),
        live: Arc::new(AtomicUsize::new(0)),
        peak: Arc::new(AtomicUsize::new(0)),
    };
    let bound = vec![(selected[0].clone(), good), (selected[1].clone(), bad)];
    let runs = tokio::time::timeout(
        Duration::from_secs(30),
        build_lanes(&vcs, &MechanicalRewrite, &gate, &sha, bound),
    )
    .await
    .expect("no lane may block on a sibling that never bound");

    assert_eq!(runs.len(), 2, "an unbound lane must still yield a run");
    assert_eq!(
        runs[1].shard.unit, selected[1].unit,
        "runs must stay in input order, or the report names the wrong shard"
    );
    assert!(
        runs[1].rewrite.is_err(),
        "the unbound lane's run must carry the refusal, not a silent pass"
    );
    assert!(runs[0].rewrite.is_ok(), "the bound lane must still build");
}
