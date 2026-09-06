//! Sharding is the Rosie rule made mechanical: one pull request per unit
//! and rule, never splitting a crate, and two shards that share an owner or
//! a touched path are not independent.

use anvil::change_delivery::core::{
    LandingPolicy, MOVE_PLAN_SCHEMA_V1, Move, MoveKind, OwnerMap, ShapeMovePlan, conflict_pairs,
    select_independent, shard_plan,
};

fn mv(kind: MoveKind, from: &str, to: &str, unit: &str, rule: &str, stable: bool) -> Move {
    Move {
        kind,
        from: from.into(),
        to: to.into(),
        unit: unit.into(),
        rule_id: rule.into(),
        evidence: String::new(),
        anchor: None,
        destination_stable: stable,
        rank: 20,
    }
}

fn plan(moves: Vec<Move>) -> ShapeMovePlan {
    ShapeMovePlan {
        schema: MOVE_PLAN_SCHEMA_V1.into(),
        repo: "oyatie/oyatie".into(),
        rev: "a".repeat(40),
        spec_version: "v1".into(),
        moves,
    }
}

fn owners() -> OwnerMap {
    OwnerMap::from_codeowners(
        "* @council\niam/ @team-iam\nbilling/ @team-billing\nk8s/ @team-iam\n",
    )
}

#[test]
fn mixed_units_and_rules_yield_one_shard_per_unit_rule() {
    let p = plan(vec![
        mv(
            MoveKind::MoveFile,
            "iam/observability/slos/a.yaml",
            "iam/slos/a.yaml",
            "iam",
            "satellite_alias_used",
            true,
        ),
        mv(
            MoveKind::MoveFile,
            "iam/observability/slos/b.yaml",
            "iam/slos/b.yaml",
            "iam",
            "satellite_alias_used",
            true,
        ),
        mv(
            MoveKind::MoveFile,
            "iam/policies/p.json",
            "iam/policy/p.json",
            "iam",
            "satellite_alias_used",
            true,
        ),
        mv(
            MoveKind::MoveFile,
            "billing/policies/q.json",
            "billing/policy/q.json",
            "billing",
            "satellite_alias_used",
            true,
        ),
        mv(
            MoveKind::CreateSkeleton,
            "",
            "billing/ports/",
            "billing",
            "unit_missing_face",
            true,
        ),
    ]);
    let shards = shard_plan(&p, &owners(), &[], &LandingPolicy::default());
    let ids: Vec<(String, String, usize)> = shards
        .iter()
        .map(|s| (s.unit.clone(), s.rule_id.clone(), s.moves.len()))
        .collect();
    assert_eq!(
        ids,
        vec![
            ("billing".into(), "satellite_alias_used".into(), 1),
            ("billing".into(), "unit_missing_face".into(), 1),
            ("iam".into(), "satellite_alias_used".into(), 3),
        ]
    );
    let iam = shards.iter().find(|s| s.unit == "iam").unwrap();
    assert_eq!(iam.owners, ["@team-iam".to_string()].into_iter().collect());
}

#[test]
fn shards_conflict_on_shared_owner_or_shared_path_and_select_respects_both() {
    let p = plan(vec![
        mv(
            MoveKind::MoveFile,
            "iam/policies/p.json",
            "iam/policy/p.json",
            "iam",
            "satellite_alias_used",
            true,
        ),
        mv(
            MoveKind::MoveFile,
            "k8s/policies/p.json",
            "k8s/policy/p.json",
            "k8s",
            "satellite_alias_used",
            true,
        ),
        mv(
            MoveKind::MoveFile,
            "billing/policies/q.json",
            "billing/policy/q.json",
            "billing",
            "satellite_alias_used",
            true,
        ),
    ]);
    let shards = shard_plan(&p, &owners(), &[], &LandingPolicy::default());
    // iam and k8s share @team-iam -> conflicting; billing is independent of both.
    let pairs = conflict_pairs(&shards);
    assert_eq!(pairs.len(), 1, "{pairs:?}");
    let chosen = select_independent(&shards, &[], &LandingPolicy::default());
    assert_eq!(
        chosen.len(),
        2,
        "max_open=2 and only two are mutually independent"
    );
    let units: Vec<&str> = chosen.iter().map(|s| s.unit.as_str()).collect();
    assert!(units.contains(&"billing"));
    assert!(!(units.contains(&"iam") && units.contains(&"k8s")));
}

#[test]
fn a_crate_move_is_never_split_below_the_crate_and_predicts_its_manifest() {
    let p = plan(vec![mv(
        MoveKind::MoveDir,
        "libs/thing/",
        "iam/core/thing/",
        "iam",
        "file_misplaced",
        true,
    )]);
    let manifests = vec![
        "libs/thing/Cargo.toml".to_string(),
        "libs/other/Cargo.toml".to_string(),
    ];
    let shards = shard_plan(&p, &owners(), &manifests, &LandingPolicy::default());
    assert_eq!(shards.len(), 1);
    assert!(shards[0].touch_set.contains("libs/thing/Cargo.toml"));
    assert!(!shards[0].touch_set.contains("libs/other/Cargo.toml"));
}

#[test]
fn a_group_over_the_cap_splits_by_subdirectory_not_by_file() {
    let moves: Vec<Move> = (0..5)
        .flat_map(|d| {
            (0..10).map(move |i| {
                mv(
                    MoveKind::MoveFile,
                    &format!("iam/legacy{d}/f{i}.rs"),
                    &format!("iam/core/x/f{d}_{i}.rs"),
                    "iam",
                    "file_misplaced",
                    true,
                )
            })
        })
        .collect();
    let policy = LandingPolicy {
        max_files_per_pr: 25,
        ..LandingPolicy::default()
    };
    let shards = shard_plan(&plan(moves), &owners(), &[], &policy);
    assert!(
        shards.len() >= 2 && shards.iter().all(|s| s.moves.len() <= 25),
        "{:?}",
        shards.iter().map(|s| s.moves.len()).collect::<Vec<_>>()
    );
}

#[test]
fn hot_file_shards_serialise_and_unstable_units_are_held() {
    let policy = LandingPolicy {
        hot_files: ["Cargo.lock".to_string()].into_iter().collect(),
        max_open_shape_prs: 5,
        one_unit_at_a_time: false,
        ..LandingPolicy::default()
    };
    let p = plan(vec![
        mv(
            MoveKind::RenameCrate,
            "Cargo.lock",
            "Cargo.lock",
            "iam",
            "crate_name_prefix",
            true,
        ),
        mv(
            MoveKind::RenameCrate,
            "Cargo.lock",
            "Cargo.lock",
            "billing",
            "crate_name_prefix",
            true,
        ),
        mv(
            MoveKind::MoveFile,
            "k8s/policies/p.json",
            "k8s/policy/p.json",
            "k8s",
            "satellite_alias_used",
            false,
        ),
    ]);
    let shards = shard_plan(&p, &OwnerMap::default(), &[], &policy);
    let chosen = select_independent(&shards, &[], &policy);
    assert_eq!(
        chosen.iter().filter(|s| s.touches_hot_file).count(),
        1,
        "one hot shard at a time"
    );
    assert!(
        chosen.iter().all(|s| s.unit != "k8s"),
        "destination not stable -> held"
    );
}

#[test]
fn shard_keys_are_stable_under_move_order_and_change_with_spec_version() {
    let a = plan(vec![
        mv(
            MoveKind::MoveFile,
            "iam/policies/p.json",
            "iam/policy/p.json",
            "iam",
            "r",
            true,
        ),
        mv(
            MoveKind::MoveFile,
            "iam/policies/q.json",
            "iam/policy/q.json",
            "iam",
            "r",
            true,
        ),
    ]);
    let mut b = a.clone();
    b.moves.reverse();
    let ka = shard_plan(&a, &OwnerMap::default(), &[], &LandingPolicy::default())[0]
        .key
        .clone();
    let kb = shard_plan(&b, &OwnerMap::default(), &[], &LandingPolicy::default())[0]
        .key
        .clone();
    assert_eq!(ka, kb);
    let mut c = a.clone();
    c.spec_version = "v2".into();
    let kc = shard_plan(&c, &OwnerMap::default(), &[], &LandingPolicy::default())[0]
        .key
        .clone();
    assert_ne!(ka, kc);
}
