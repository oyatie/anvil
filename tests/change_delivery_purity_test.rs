//! I8: a structure-only pull request changes structure only. The purity
//! check reads the staged diff; anything that is not a rename, a wiring
//! line, or a templated skeleton file fails closed.

use anvil::change_delivery::core::{
    LandingPolicy, MOVE_PLAN_SCHEMA_V1, Move, MoveKind, NameStatus, OwnerMap, PurityViolation,
    ShapeMovePlan, Shard, diff_is_structure_only, shard_plan,
};

fn shard(kind: MoveKind, from: &str, to: &str) -> Shard {
    let plan = ShapeMovePlan {
        schema: MOVE_PLAN_SCHEMA_V1.into(),
        repo: "r".into(),
        rev: "a".repeat(40),
        spec_version: "v1".into(),
        moves: vec![Move {
            kind,
            from: from.into(),
            to: to.into(),
            unit: "iam".into(),
            rule_id: "file_misplaced".into(),
            evidence: String::new(),
            anchor: None,
            destination_stable: true,
            rank: 1,
        }],
    };
    shard_plan(&plan, &OwnerMap::default(), &[], &LandingPolicy::default()).remove(0)
}

#[test]
fn a_rename_plus_wiring_edits_is_pure() {
    let s = shard(MoveKind::MoveFile, "iam/old/a.rs", "iam/core/a.rs");
    let ns = NameStatus::parse("R100\tiam/old/a.rs\tiam/core/a.rs\nM\tiam/lib.rs\n");
    let diff = "diff --git a/iam/lib.rs b/iam/lib.rs\n--- a/iam/lib.rs\n+++ b/iam/lib.rs\n@@ -1,2 +1,2 @@\n-mod old;\n+mod core;\n-use crate::old::a::Thing;\n+use crate::core::a::Thing;\n";
    assert_eq!(diff_is_structure_only(&ns, diff, &s), Ok(()));
}

#[test]
fn a_behaviour_line_in_a_modified_file_is_rejected() {
    let s = shard(MoveKind::MoveFile, "iam/old/a.rs", "iam/core/a.rs");
    let ns = NameStatus::parse("R100\tiam/old/a.rs\tiam/core/a.rs\nM\tiam/lib.rs\n");
    let diff = "+++ b/iam/lib.rs\n@@\n-use crate::old::a::Thing;\n+use crate::core::a::Thing;\n-    if x > 1 {\n+    if x > 2 {\n";
    let err = diff_is_structure_only(&ns, diff, &s).unwrap_err();
    assert!(err.iter().any(|v| matches!(v, PurityViolation::BehaviourLineChanged { line, .. } if line.contains("x > 2"))), "{err:?}");
}

#[test]
fn low_similarity_renames_deletions_and_conflict_markers_are_rejected() {
    let s = shard(MoveKind::MoveFile, "iam/old/a.rs", "iam/core/a.rs");
    let ns =
        NameStatus::parse("R040\tiam/old/a.rs\tiam/core/a.rs\nD\tiam/gone.rs\nM\tiam/lib.rs\n");
    let diff = "+++ b/iam/lib.rs\n@@\n+<<<<<<< HEAD\n+use a;\n";
    let err = diff_is_structure_only(&ns, diff, &s).unwrap_err();
    assert!(err.iter().any(|v| matches!(
        v,
        PurityViolation::LowSimilarityRename { similarity: 40, .. }
    )));
    assert!(
        err.iter()
            .any(|v| matches!(v, PurityViolation::Deletion { .. }))
    );
    assert!(
        err.iter()
            .any(|v| matches!(v, PurityViolation::ConflictMarker { .. })),
        "{err:?}"
    );
}

#[test]
fn an_addition_is_allowed_only_for_a_templated_skeleton_path() {
    let s = shard(MoveKind::CreateSkeleton, "", "iam/ports/");
    let ok = NameStatus::parse("A\tiam/ports/\n");
    assert_eq!(diff_is_structure_only(&ok, "", &s), Ok(()));
    let stray = NameStatus::parse("A\tiam/ports/\nA\tiam/helper.rs\n");
    let err = diff_is_structure_only(&stray, "", &s).unwrap_err();
    assert_eq!(
        err,
        vec![PurityViolation::UnexpectedAddition {
            path: "iam/helper.rs".into()
        }]
    );
    let s2 = shard(MoveKind::MoveFile, "a", "b");
    let err = diff_is_structure_only(&NameStatus::parse("A\tb\n"), "", &s2).unwrap_err();
    assert!(
        matches!(err[0], PurityViolation::UnexpectedAddition { .. }),
        "a move shard may not add files"
    );
}
