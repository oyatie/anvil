//! Dry-run delivery: for each selected shard, build the change in an
//! isolated lane worktree — rewrite, stage, purity-check, gate — print the
//! result, and tear the lane down. Nothing is committed, nothing is pushed;
//! opening pull requests is a later change with its own review.
//!
//! The selected shards are built concurrently. `select_independent` exists to
//! produce a set that is touch- and owner-disjoint, and `create_lane` gives
//! each one its own worktree; running that set one at a time throws the
//! guarantee away and costs one lane gate (`cargo check`, bounded at 1800s)
//! per shard in series. N disjoint shards are N lanes, not one lane run N
//! times.

use crate::change_delivery::adapters::{CargoGate, GitLaneVcs, MechanicalRewrite};
use crate::change_delivery::facade::plan::{
    manifests_from_tree, owners_from_tree, plan_from_report,
};
use crate::change_delivery::ports::{
    GateResult, LandingPolicy, LaneError, LaneWorktree, LocalGate, PurityViolation, RewriteEngine,
    Shard, VcsPort, select_independent, shard_plan,
};
use crate::shape::adapters::GitTreeAtRev;
use crate::shape::facade::measure::{MeasureRequest, measure_repo};
use crate::shape::ports::TreeSource;
use anyhow::{Result, anyhow};
use futures::future::join_all;
use std::path::{Path, PathBuf};

/// Anvil's own config path for landing policy inside a tenant repository.
pub const LANDING_PATH: &str = ".anvil/landing.json";

#[derive(Debug)]
pub struct ShardRun {
    pub shard: Shard,
    pub lane_base: String,
    pub rewrite: Result<(), String>,
    pub purity: Option<Result<(), Vec<PurityViolation>>>,
    pub gate: Option<GateResult>,
    pub diffstat: String,
}

pub struct DeliverRequest {
    pub repo_dir: PathBuf,
    pub repo: String,
    pub max: usize,
    pub spec_override: Option<PathBuf>,
    /// Dry-runs on the operator's own checkout only; every daemon path
    /// passes false and the self-source guard refuses the daemon tree.
    pub allow_same_repo: bool,
}

async fn landing_policy(repo_dir: &Path, rev: &str) -> (LandingPolicy, Option<String>) {
    let bytes = match GitTreeAtRev::load(repo_dir, rev, |p| p == LANDING_PATH).await {
        Ok(tree) => tree.read(LANDING_PATH).ok().flatten().map(|b| b.to_vec()),
        Err(_) => None,
    };
    LandingPolicy::load(bytes.as_deref())
}

pub async fn deliver_dry_run(
    req: &DeliverRequest,
) -> Result<(Vec<ShardRun>, Vec<Shard>, LandingPolicy)> {
    let sha = GitTreeAtRev::resolve(&req.repo_dir, "HEAD")
        .await
        .map_err(|e| anyhow!("{e}"))?;
    let report = measure_repo(&MeasureRequest {
        repo_dir: req.repo_dir.clone(),
        rev: sha.clone(),
        repo: req.repo.clone(),
        spec_override: req.spec_override.clone(),
        registry_override: None,
    })
    .await?;
    let spec_version = format!("{:?}", report.spec_source);
    let plan = plan_from_report(&report, &spec_version);
    let owners = owners_from_tree(&req.repo_dir, &sha).await;
    let manifests = manifests_from_tree(&req.repo_dir, &sha).await;
    let (policy, policy_problem) = landing_policy(&req.repo_dir, &sha).await;
    if let Some(p) = policy_problem {
        tracing::warn!("{p}");
    }
    let shards = shard_plan(&plan, &owners, &manifests, &policy);
    let mut selected = select_independent(&shards, &[], &policy);
    selected.truncate(req.max);

    let vcs = GitLaneVcs::new(req.repo_dir.join(".anvil-lanes"));
    let rewrite = MechanicalRewrite;
    let gate = CargoGate;

    // Binding lanes is serial on purpose: `git worktree add` mutates the
    // repository's shared administrative directory, and two of them racing
    // there is a corrupt worktree list rather than a faster dry run. It is
    // also the cheap half — the cost of a lane is its gate, not its checkout.
    let mut bound: Vec<(Shard, Result<LaneWorktree, LaneError>)> = Vec::new();
    for shard in selected {
        let lane_id = format!(
            "{}-g{}",
            &shard.key.0[..8.min(shard.key.0.len())],
            shard.generation
        );
        let lane = vcs
            .create_lane(&req.repo_dir, &lane_id, &sha, req.allow_same_repo)
            .await;
        bound.push((shard, lane));
    }

    let runs = build_lanes(&vcs, &rewrite, &gate, &sha, bound).await;
    Ok((runs, shards, policy))
}

/// Builds every bound lane concurrently and returns one run per input, in
/// input order.
///
/// Concurrent because the shards are disjoint, not merely because it is
/// faster: `select_independent` has already established that no two of them
/// share a touched path or an owner, and each holds its own worktree, so
/// there is nothing for them to serialise on. Folding them onto one worker
/// spends `max_open_shape_prs` gate runs in series to produce a result that
/// is identical except for how long the operator waited.
///
/// A lane that could not be bound still yields a run, in its place, saying
/// why — a shard that vanished from the report would be a shard nobody knows
/// was refused.
pub async fn build_lanes(
    vcs: &dyn VcsPort,
    rewrite: &dyn RewriteEngine,
    gate: &dyn LocalGate,
    base_rev: &str,
    bound: Vec<(Shard, Result<LaneWorktree, LaneError>)>,
) -> Vec<ShardRun> {
    join_all(bound.into_iter().map(|(shard, lane)| async move {
        match lane {
            Err(e) => ShardRun {
                shard,
                lane_base: base_rev.to_string(),
                rewrite: Err(e.to_string()),
                purity: None,
                gate: None,
                diffstat: String::new(),
            },
            Ok(lane) => build_one_lane(vcs, rewrite, gate, shard, lane).await,
        }
    }))
    .await
}

/// Rewrite, stage, purity-check and gate one lane, then tear it down.
async fn build_one_lane(
    vcs: &dyn VcsPort,
    rewrite: &dyn RewriteEngine,
    gate: &dyn LocalGate,
    shard: Shard,
    lane: LaneWorktree,
) -> ShardRun {
    let mut run = ShardRun {
        shard: shard.clone(),
        lane_base: lane.base_rev.clone(),
        rewrite: Ok(()),
        purity: None,
        gate: None,
        diffstat: String::new(),
    };
    match rewrite.apply(vcs, &lane, &shard).await {
        Err(e) => run.rewrite = Err(e.to_string()),
        Ok(()) => match vcs.stage(&lane).await {
            Err(e) => run.rewrite = Err(e.to_string()),
            Ok(()) => {
                let ns = vcs.name_status(&lane).await.unwrap_or_default();
                let diff = vcs.cached_diff(&lane).await.unwrap_or_default();
                run.purity = Some(crate::change_delivery::ports::diff_is_structure_only(
                    &ns, &diff, &shard,
                ));
                run.diffstat = vcs.diffstat(&lane).await.unwrap_or_default();
                run.gate = Some(gate.run(&lane).await);
            }
        },
    }
    let _ = vcs.cleanup(lane).await;
    run
}

pub fn render(runs: &[ShardRun], total_shards: usize, policy: &LandingPolicy) -> String {
    let mut out = format!(
        "deliver (dry run): {} shard(s) planned, {} built in lanes (mode {:?}; nothing committed, nothing pushed)\n",
        total_shards,
        runs.len(),
        policy.mode
    );
    for r in runs {
        out.push_str(&format!(
            "  [{}] {} / {} — rewrite: {}; purity: {}; gate: {}\n",
            &r.shard.key.0[..8.min(r.shard.key.0.len())],
            r.shard.unit,
            r.shard.rule_id,
            match &r.rewrite {
                Ok(()) => "ok".to_string(),
                Err(e) => format!("REFUSED ({e})"),
            },
            match &r.purity {
                None => "not reached".to_string(),
                Some(Ok(())) => "structure-only".to_string(),
                Some(Err(v)) => format!("IMPURE ({} violation(s))", v.len()),
            },
            match &r.gate {
                None => "not reached".to_string(),
                Some(GateResult::Passed { label }) => format!("{label} passed"),
                Some(GateResult::Failed { label, .. }) => format!("{label} FAILED"),
                Some(GateResult::Unavailable { reason }) => format!("unavailable ({reason})"),
            }
        ));
        for line in r.diffstat.lines().take(6) {
            out.push_str(&format!("      {line}\n"));
        }
    }
    out
}
