//! From a shape report to a ranked move plan to a dry-run shard listing.
//! Touches no network; reads the tenant's ownership files and landing
//! policy from the measured tree when they exist.

use crate::change_delivery::core::{
    LandingPolicy, MOVE_PLAN_SCHEMA_V1, Move, MoveKind, OwnerMap, ShapeMovePlan, Shard,
    conflict_pairs, select_independent, shard_plan,
};
use crate::shape::ports::{Fix, ShapeReport};

/// Rank: stable units first; satellite alias moves (mechanical, no code)
/// before other file moves; crate renames last.
fn rank(unit_stable: bool, rule: &str, kind: MoveKind) -> u32 {
    let base = match kind {
        MoveKind::CreateSkeleton | MoveKind::AddManifest => 10,
        MoveKind::MoveFile if rule == "satellite_alias_used" => 20,
        MoveKind::MoveFile | MoveKind::SplitSatellite => 30,
        MoveKind::MoveDir => 40,
        MoveKind::RenameCrate => 50,
    };
    if unit_stable { base } else { base + 100 }
}

pub fn plan_from_report(report: &ShapeReport, spec_version: &str) -> ShapeMovePlan {
    let stable: std::collections::BTreeMap<&str, bool> = report
        .units
        .iter()
        .map(|u| (u.unit.as_str(), u.destination_stable))
        .collect();
    let mut moves = Vec::new();
    for f in &report.findings {
        let unit = f.unit.clone().unwrap_or_default();
        let unit_stable = stable.get(unit.as_str()).copied().unwrap_or(false);
        let (kind, from, to) = match &f.fix {
            Some(Fix::Move { from, to }) => (
                if from.ends_with('/') {
                    MoveKind::MoveDir
                } else {
                    MoveKind::MoveFile
                },
                from.clone(),
                to.clone(),
            ),
            Some(Fix::Rename { from, to }) => (MoveKind::RenameCrate, from.clone(), to.clone()),
            Some(Fix::Create { path }) => (MoveKind::CreateSkeleton, String::new(), path.clone()),
            _ => continue,
        };
        moves.push(Move {
            rank: rank(unit_stable, &f.rule.0, kind),
            kind,
            from,
            to,
            unit,
            rule_id: f.rule.0.clone(),
            evidence: f.detail.clone(),
            anchor: Some(f.path.clone()),
            destination_stable: unit_stable,
        });
    }
    moves.sort_by(|a, b| {
        (a.rank, &a.unit, &a.rule_id, &a.from).cmp(&(b.rank, &b.unit, &b.rule_id, &b.from))
    });
    ShapeMovePlan {
        schema: MOVE_PLAN_SCHEMA_V1.to_string(),
        repo: report.repo.clone(),
        rev: report.rev.clone(),
        spec_version: spec_version.to_string(),
        moves,
    }
}

pub struct DryRun {
    pub shards: Vec<Shard>,
    pub selected: Vec<Shard>,
    pub conflicts: usize,
    pub policy: LandingPolicy,
}

pub fn dry_run(
    plan: &ShapeMovePlan,
    owners: &OwnerMap,
    manifests: &[String],
    policy: LandingPolicy,
) -> DryRun {
    let shards = shard_plan(plan, owners, manifests, &policy);
    let conflicts = conflict_pairs(&shards).len();
    let selected = select_independent(&shards, &[], &policy);
    DryRun {
        shards,
        selected,
        conflicts,
        policy,
    }
}

pub fn render(d: &DryRun, plan: &ShapeMovePlan) -> String {
    let held = d.shards.iter().filter(|s| !s.destination_stable).count();
    let hot = d.shards.iter().filter(|s| s.touches_hot_file).count();
    let mut out = format!(
        "plan: {} @ {} — {} move(s) -> {} shard(s); {} conflicting pair(s); {} held (destination not stable); {} touch hot files\n  policy: mode={:?} max_open={} max_files={} require_destination_stable={}\n  would open now: {}\n",
        plan.repo,
        &plan.rev[..plan.rev.len().min(12)],
        plan.moves.len(),
        d.shards.len(),
        d.conflicts,
        held,
        hot,
        d.policy.mode,
        d.policy.max_open_shape_prs,
        d.policy.max_files_per_pr,
        d.policy.require_destination_stable,
        d.selected.len()
    );
    for s in d.shards.iter().take(25) {
        out.push_str(&format!(
            "  [{}] {:<28} {:<20} {:>3} move(s)  owners={:?}{}{}\n",
            &s.key.0[..8],
            s.rule_id,
            s.unit,
            s.moves.len(),
            s.owners,
            if s.touches_hot_file { "  HOT" } else { "" },
            if s.destination_stable { "" } else { "  held" }
        ));
    }
    if d.shards.len() > 25 {
        out.push_str(&format!("  ... {} more shard(s)\n", d.shards.len() - 25));
    }
    out
}

/// Ownership from the measured tree: `.github/CODEOWNERS` (or root
/// `CODEOWNERS`) plus every `OWNERS` file, read by revision.
pub async fn owners_from_tree(repo_dir: &std::path::Path, rev: &str) -> OwnerMap {
    let mut map = OwnerMap::default();
    let Ok(tree) = crate::shape::adapters::GitTreeAtRev::load(repo_dir, rev, |p| {
        p == ".github/CODEOWNERS" || p == "CODEOWNERS" || p.rsplit('/').next() == Some("OWNERS")
    })
    .await
    else {
        return map;
    };
    use crate::shape::ports::TreeSource;
    for (path, bytes) in tree.loaded() {
        let text = String::from_utf8_lossy(bytes);
        if path.ends_with("CODEOWNERS") {
            map.add_codeowners(&text);
        } else {
            let dir = path
                .rsplit_once('/')
                .map(|(d, _)| d.to_string())
                .unwrap_or_default();
            map.add_owners_file(&dir, &text);
        }
    }
    map
}

/// Manifest paths (every profile's unit marker) at the revision, for
/// touch-set prediction.
pub async fn manifests_from_tree(repo_dir: &std::path::Path, rev: &str) -> Vec<String> {
    use crate::shape::ports::{LanguageProfile, TreeSource};
    let markers: Vec<&str> = LanguageProfile::ALL
        .iter()
        .map(|p| p.unit_marker())
        .collect();
    match crate::shape::adapters::GitTreeAtRev::load(repo_dir, rev, |_| false).await {
        Ok(tree) => tree
            .paths()
            .iter()
            .filter(|p| p.rsplit('/').next().is_some_and(|b| markers.contains(&b)))
            .cloned()
            .collect(),
        Err(_) => Vec::new(),
    }
}
