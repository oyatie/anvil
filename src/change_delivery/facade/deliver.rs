//! Dry-run delivery: for each selected shard, build the change in an
//! isolated lane worktree — rewrite, stage, purity-check, gate — print the
//! result, and tear the lane down. Nothing is committed, nothing is pushed;
//! opening pull requests is a later change with its own review.

use crate::change_delivery::adapters::{CargoGate, GitLaneVcs, MechanicalRewrite};
use crate::change_delivery::facade::plan::{
    manifests_from_tree, owners_from_tree, plan_from_report,
};
use crate::change_delivery::ports::{
    GateResult, LandingPolicy, LocalGate, PurityViolation, RewriteEngine, Shard, VcsPort, sequence,
    shard_plan,
};
use crate::shape::facade::GitTreeAtRev;
use crate::shape::facade::TreeSource;
use crate::shape::facade::measure::{MeasureRequest, measure_repo};
use anyhow::{Result, anyhow};
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
        repo_dir: crate::git_manager::SubjectRoot::asserted(
            req.repo_dir.clone(),
            crate::git_manager::Uncloned::OperatorSupplied,
        ),
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
    // The first wave is what may open now; the rest are rounds behind it, and
    // are reported rather than dropped. `truncate` bounds this run, not the
    // plan -- a shard cut here comes back in the next dispatch instead of
    // vanishing from the account of what the plan contains.
    let sequenced = sequence(&shards, &[], &policy);
    let mut selected = sequenced.waves.first().cloned().unwrap_or_default();
    selected.truncate(req.max);
    if sequenced.waves.len() > 1 || !sequenced.held.is_empty() || !sequenced.stuck.is_empty() {
        tracing::info!(
            rounds = sequenced.waves.len(),
            placed = sequenced.placed(),
            held = sequenced.held.len(),
            stuck = sequenced.stuck.len(),
            opening_now = selected.len(),
            "shape delivery is sequenced across rounds"
        );
    }

    let vcs = GitLaneVcs::new(req.repo_dir.join(".anvil-lanes"));
    let rewrite = MechanicalRewrite;
    let gate = CargoGate;
    let mut runs = Vec::new();
    for shard in selected {
        let lane_id = format!(
            "{}-g{}",
            &shard.key.0[..8.min(shard.key.0.len())],
            shard.generation
        );
        let lane = match vcs
            .create_lane(&req.repo_dir, &lane_id, &sha, req.allow_same_repo)
            .await
        {
            Ok(l) => l,
            Err(e) => {
                runs.push(ShardRun {
                    shard,
                    lane_base: sha.clone(),
                    rewrite: Err(e.to_string()),
                    purity: None,
                    gate: None,
                    diffstat: String::new(),
                });
                continue;
            }
        };
        let mut run = ShardRun {
            shard: shard.clone(),
            lane_base: lane.base_rev.clone(),
            rewrite: Ok(()),
            purity: None,
            gate: None,
            diffstat: String::new(),
        };
        match rewrite.apply(&vcs, &lane, &shard).await {
            Err(e) => run.rewrite = Err(e.to_string()),
            Ok(()) => {
                let staged = vcs.stage(&lane).await;
                match staged {
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
                }
            }
        }
        let _ = vcs.cleanup(lane).await;
        runs.push(run);
    }
    Ok((runs, shards, policy))
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
