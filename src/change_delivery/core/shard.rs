//! Sharding: one pull request per (unit, rule); never below crate
//! granularity; owner- and touch-disjoint shards are independent (Rosie,
//! SubmitQueue); at most one shard touching a hot file in flight.

use super::model::{Move, MoveKind, ShapeMovePlan, Shard};
use super::naming::shard_key;
use super::owners::OwnerMap;
use super::policy::LandingPolicy;

#[path = "occupancy.rs"]
pub mod occupancy;
pub use occupancy::{
    admit_spawn, anvil_hubs, is_open_test_crate, occupy_move, path_sets_disjoint, SpawnKind,
    SpawnRefused,
};
use std::collections::{BTreeMap, BTreeSet};

/// Paths the rewrite is predicted to touch besides the moved ones: the
/// crate manifest of a moved crate directory, and any manifest under the
/// unit root for a crate rename. Kept conservative; purity catches the rest.
fn predicted_touch(m: &Move, manifests: &[String]) -> BTreeSet<String> {
    let mut t = BTreeSet::new();
    for p in [&m.from, &m.to].into_iter().chain(m.anchor.iter()) {
        if !p.is_empty() {
            t.insert(p.clone());
        }
    }
    match m.kind {
        MoveKind::MoveDir | MoveKind::RenameCrate => {
            for p in manifests {
                if p.starts_with(m.from.trim_end_matches('/')) {
                    t.insert(p.clone());
                }
            }
        }
        _ => {}
    }
    t
}

pub fn shard_plan(
    plan: &ShapeMovePlan,
    owners: &OwnerMap,
    manifests: &[String],
    policy: &LandingPolicy,
) -> Vec<Shard> {
    let mut groups: BTreeMap<(String, String), Vec<Move>> = BTreeMap::new();
    for m in &plan.moves {
        groups
            .entry((m.unit.clone(), m.rule_id.clone()))
            .or_default()
            .push(m.clone());
    }
    let mut shards = Vec::new();
    for ((unit, rule_id), moves) in groups {
        for chunk in split_by_cap(moves, policy.max_files_per_pr as usize) {
            let mut touch = BTreeSet::new();
            for m in &chunk {
                touch.extend(predicted_touch(m, manifests));
            }
            let mut owner_set = BTreeSet::new();
            for p in &touch {
                owner_set.extend(owners.owners_of(p));
            }
            let touches_hot_file = touch.iter().any(|p| policy.hot_files.contains(p));
            let destination_stable = chunk.iter().all(|m| m.destination_stable);
            let key = shard_key(&plan.repo, &rule_id, &unit, &chunk, &plan.spec_version);
            shards.push(Shard {
                key,
                repo: plan.repo.clone(),
                unit: unit.clone(),
                rule_id: rule_id.clone(),
                spec_version: plan.spec_version.clone(),
                moves: chunk,
                touch_set: touch,
                owners: owner_set,
                touches_hot_file,
                destination_stable,
                generation: 1,
            });
        }
    }
    shards.sort_by_key(|s| (s.rank(), s.unit.clone(), s.rule_id.clone()));
    shards
}

/// Splits a group above the cap by the first path segment below the unit
/// root, never splitting a crate directory (a crate move is one Move).
fn split_by_cap(moves: Vec<Move>, cap: usize) -> Vec<Vec<Move>> {
    if cap == 0 || moves.len() <= cap {
        return vec![moves];
    }
    let mut by_sub: BTreeMap<String, Vec<Move>> = BTreeMap::new();
    for m in moves {
        let sub = m.from.split('/').take(2).collect::<Vec<_>>().join("/");
        by_sub.entry(sub).or_default().push(m);
    }
    let mut out: Vec<Vec<Move>> = Vec::new();
    let mut cur: Vec<Move> = Vec::new();
    for (_, mut group) in by_sub {
        if !cur.is_empty() && cur.len() + group.len() > cap {
            out.push(std::mem::take(&mut cur));
        }
        cur.append(&mut group);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

pub fn conflicts(a: &Shard, b: &Shard) -> bool {
    a.touch_set.intersection(&b.touch_set).next().is_some()
        || a.owners.intersection(&b.owners).next().is_some()
}

pub fn conflict_pairs(shards: &[Shard]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for i in 0..shards.len() {
        for j in i + 1..shards.len() {
            if conflicts(&shards[i], &shards[j]) {
                out.push((i, j));
            }
        }
    }
    out
}

/// A maximal independent set by rank: disjoint from everything in flight
/// and from each other, honouring destination_stable, hot-file serialisation
/// and the open-PR cap.
pub fn select_independent(
    shards: &[Shard],
    in_flight: &[Shard],
    policy: &LandingPolicy,
) -> Vec<Shard> {
    let open = in_flight.len() as u32;
    let room = policy.max_open_shape_prs.saturating_sub(open) as usize;
    let mut hot_in_flight = in_flight.iter().any(|s| s.touches_hot_file);
    let units_in_flight: BTreeSet<&str> = in_flight.iter().map(|s| s.unit.as_str()).collect();
    let mut chosen: Vec<Shard> = Vec::new();
    for s in shards {
        if chosen.len() >= room {
            break;
        }
        if policy.require_destination_stable && !s.destination_stable {
            continue;
        }
        if s.touches_hot_file && hot_in_flight {
            continue;
        }
        if policy.one_unit_at_a_time
            && (units_in_flight.contains(s.unit.as_str())
                || chosen.iter().any(|c| c.unit == s.unit))
        {
            continue;
        }
        if in_flight
            .iter()
            .chain(chosen.iter())
            .any(|o| conflicts(o, s))
        {
            continue;
        }
        if s.touches_hot_file {
            hot_in_flight = true;
        }
        chosen.push(s.clone());
    }
    chosen
}
