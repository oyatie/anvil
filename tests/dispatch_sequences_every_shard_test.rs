//! A shard that collides is scheduled later, not dropped.
//!
//! `select_independent` answers one question — what may open right now — and
//! says nothing about the rest. A plan of three shards where two collide
//! renders as "would open now: 2", and the third is simply absent from the dry
//! run: a reader cannot tell it from a shard that was never planned, and cannot
//! tell "waits one round" from "excluded for good".
//!
//! That is the difference between sequencing and rejecting. Overlap is avoided
//! at spawn by putting the second lane in the next wave, not by punishing it at
//! CI or by quietly shortening the list.

use anvil::change_delivery::ports::{
    Held, LandingPolicy, Move, MoveKind, Shard, ShardKey, select_independent, sequence,
};
use std::collections::BTreeSet;

fn set(v: &[&str]) -> BTreeSet<String> {
    v.iter().map(|s| (*s).to_string()).collect()
}

fn a_move(from: &str, to: &str) -> Move {
    Move {
        rank: 10,
        kind: MoveKind::MoveFile,
        from: from.to_string(),
        to: to.to_string(),
        unit: "u".to_string(),
        rule_id: "r".to_string(),
        evidence: String::new(),
        anchor: None,
        destination_stable: true,
    }
}

fn shard(key: &str, unit: &str, touches: &[&str], owners: &[&str]) -> Shard {
    Shard {
        key: ShardKey(key.to_string()),
        repo: "oyatie/console".to_string(),
        unit: unit.to_string(),
        rule_id: "r".to_string(),
        spec_version: "v1".to_string(),
        moves: vec![a_move(touches[0], touches[0])],
        touch_set: set(touches),
        owners: set(owners),
        touches_hot_file: false,
        destination_stable: true,
        generation: 0,
    }
}

/// Nothing in this repository's policy defaults may make the fixtures vacuous.
fn policy() -> LandingPolicy {
    LandingPolicy {
        require_destination_stable: true,
        one_unit_at_a_time: false,
        max_open_shape_prs: 10,
        ..Default::default()
    }
}

#[test]
fn two_shards_on_one_path_land_in_consecutive_waves() {
    let shards = [
        shard("aaaaaaaa", "u1", &["src/a.rs"], &["@team"]),
        shard("bbbbbbbb", "u2", &["src/a.rs"], &["@other"]),
    ];

    // What today's dispatch sees: one of them, and no word about the other.
    let now = select_independent(&shards, &[], &policy());
    assert_eq!(now.len(), 1, "fixture sanity: these two do collide");

    let seq = sequence(&shards, &[], &policy());
    assert_eq!(
        seq.waves.len(),
        2,
        "a colliding pair is two rounds, not one round and a disappearance"
    );
    assert_eq!(seq.placed(), 2, "every shard is placed somewhere");
    assert!(seq.held.is_empty() && seq.stuck.is_empty());
    assert_eq!(seq.waves[0][0].key.0, "aaaaaaaa");
    assert_eq!(seq.waves[1][0].key.0, "bbbbbbbb");
}

#[test]
fn disjoint_shards_share_one_wave() {
    let shards = [
        shard("aaaaaaaa", "u1", &["src/a.rs"], &["@team"]),
        shard("bbbbbbbb", "u2", &["src/b.rs"], &["@other"]),
    ];
    let seq = sequence(&shards, &[], &policy());
    assert_eq!(
        seq.waves.len(),
        1,
        "nothing collides, so nothing waits: {:?}",
        seq.waves.iter().map(Vec::len).collect::<Vec<_>>()
    );
    assert_eq!(seq.placed(), 2);
}

/// Owner overlap is a collision too, and it must sequence rather than vanish.
#[test]
fn shards_sharing_an_owner_land_in_consecutive_waves() {
    let shards = [
        shard("aaaaaaaa", "u1", &["src/a.rs"], &["@team"]),
        shard("bbbbbbbb", "u2", &["src/b.rs"], &["@team"]),
    ];
    let seq = sequence(&shards, &[], &policy());
    assert_eq!(seq.waves.len(), 2);
    assert_eq!(seq.placed(), 2);
}

/// The distinction the caller could not previously draw: a shard that waits,
/// and a shard that is never coming.
#[test]
fn a_shard_policy_excludes_is_reported_as_held_not_as_a_later_wave() {
    let mut unstable = shard("cccccccc", "u3", &["src/c.rs"], &["@third"]);
    unstable.destination_stable = false;
    let shards = [shard("aaaaaaaa", "u1", &["src/a.rs"], &["@team"]), unstable];

    let seq = sequence(&shards, &[], &policy());
    assert_eq!(seq.placed(), 1, "only the stable one is ever opened");
    assert_eq!(seq.held.len(), 1, "and the other is accounted for");
    assert_eq!(seq.held[0].0.key.0, "cccccccc");
    assert_eq!(seq.held[0].1, Held::DestinationNotStable);
    assert!(
        seq.stuck.is_empty(),
        "a shard excluded by policy is held, not stuck: it is not waiting for \
         anything and no later round will admit it"
    );
}

/// Three on one path is three rounds, not a cycle and not a truncation.
#[test]
fn a_chain_of_collisions_is_as_many_waves_as_it_has_links() {
    let shards = [
        shard("aaaaaaaa", "u1", &["src/a.rs"], &["@a"]),
        shard("bbbbbbbb", "u2", &["src/a.rs"], &["@b"]),
        shard("cccccccc", "u3", &["src/a.rs"], &["@c"]),
    ];
    let seq = sequence(&shards, &[], &policy());
    assert_eq!(seq.waves.len(), 3);
    assert_eq!(seq.placed(), 3);
    assert!(seq.stuck.is_empty());
}

/// Work already open holds its paths against the first wave, exactly as it
/// holds them against `select_independent`.
#[test]
fn shards_in_flight_push_a_colliding_shard_to_a_later_wave() {
    let open = [shard("dddddddd", "u9", &["src/a.rs"], &["@open"])];
    let shards = [shard("aaaaaaaa", "u1", &["src/a.rs"], &["@team"])];
    let seq = sequence(&shards, &open, &policy());
    assert_eq!(
        seq.waves.len(),
        1,
        "it still opens, one round behind what is already in flight"
    );
    assert_eq!(seq.placed(), 1);
}

/// The termination guarantee, stated as a test rather than as a comment: a
/// policy that admits nothing reports what it could not place instead of
/// looping.
#[test]
fn a_policy_that_admits_nothing_reports_the_remainder_rather_than_hanging() {
    let p = LandingPolicy {
        max_open_shape_prs: 0,
        ..policy()
    };
    let shards = [shard("aaaaaaaa", "u1", &["src/a.rs"], &["@team"])];
    let seq = sequence(&shards, &[], &p);
    assert!(seq.waves.is_empty());
    assert_eq!(seq.stuck.len(), 1, "named, not dropped and not looped on");
}
