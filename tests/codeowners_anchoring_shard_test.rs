//! Root-only CODEOWNERS rules must not distort nested shard independence.
//! Complete plans and ownership data stay in memory; no files are moved.

use anvil::change_delivery::core::{
    LandingPolicy, MOVE_PLAN_SCHEMA_V1, Move, MoveKind, OwnerMap, ShapeMovePlan, conflict_pairs,
    select_independent, shard_plan,
};

fn nested_move(unit: &str, basename: &str) -> Move {
    Move {
        kind: MoveKind::MoveFile,
        from: format!("{unit}/{basename}"),
        to: format!("{unit}/nested/{basename}"),
        unit: unit.into(),
        rule_id: "file_misplaced".into(),
        evidence: String::new(),
        anchor: None,
        destination_stable: true,
        rank: 20,
    }
}

fn plan(second_basename: &str) -> ShapeMovePlan {
    ShapeMovePlan {
        schema: MOVE_PLAN_SCHEMA_V1.into(),
        repo: "example/repo".into(),
        rev: "a".repeat(40),
        spec_version: "v1".into(),
        moves: vec![
            nested_move("a", "README.md"),
            nested_move("b", second_basename),
        ],
    }
}

#[test]
fn a_root_only_rule_cannot_hide_a_shared_nested_owner() {
    let owners = OwnerMap::from_codeowners("a/ @shared\nb/ @shared\n/README.md @root\n");
    let policy = LandingPolicy::default();
    let shards = shard_plan(&plan("other.md"), &owners, &[], &policy);
    assert_eq!(shards.len(), 2);
    assert_eq!(
        conflict_pairs(&shards).len(),
        1,
        "shared nested owner must conflict"
    );
    assert_eq!(select_independent(&shards, &[], &policy).len(), 1);
    for shard in &shards {
        assert_eq!(shard.owners, ["@shared".into()].into());
    }
}

#[test]
fn a_root_only_rule_cannot_create_a_spurious_nested_owner_conflict() {
    let owners = OwnerMap::from_codeowners("a/ @team-a\nb/ @team-b\n/README.md @root\n");
    let policy = LandingPolicy::default();
    let shards = shard_plan(&plan("README.md"), &owners, &[], &policy);
    assert_eq!(shards.len(), 2);
    assert!(
        conflict_pairs(&shards).is_empty(),
        "nested owners are independent"
    );
    assert_eq!(select_independent(&shards, &[], &policy).len(), 2);
    assert_eq!(shards[0].owners, ["@team-a".into()].into());
    assert_eq!(shards[1].owners, ["@team-b".into()].into());
}
