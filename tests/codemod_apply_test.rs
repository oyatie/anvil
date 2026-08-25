//! The codemod half. A repair only a human can apply is a repair applied 47
//! times by hand, and at 251 sites it is not applied at all.
//!
//! Every test here plants a case the planner must REFUSE rather than
//! half-apply. A codemod that skips what it cannot handle produces a tree that
//! looks migrated and is not, which is the same defect as a check reading
//! absence as success, one layer over.

use anvil::harness::apply::{Edit, Refused, plan};
use anvil::harness::{Finding, Fix};
use std::collections::BTreeMap;

fn files(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(p, b)| (p.to_string(), b.to_string()))
        .collect()
}

fn finding(subject: &str, fix: Option<Fix>) -> Finding {
    Finding {
        rule: "test_rule",
        key: subject.to_string(),
        subject: subject.to_string(),
        detail: "planted".into(),
        fix,
    }
}

#[test]
fn a_rename_becomes_a_concrete_edit() {
    let f = finding(
        "bus/adapters/file/Cargo.toml",
        Some(Fix::RenameSymbol {
            from: "messaging-file-adapter".into(),
            to: "bus-file".into(),
        }),
    );
    let tree = files(&[(
        "bus/adapters/file/Cargo.toml",
        "[package]\nname = \"messaging-file-adapter\"\n",
    )]);
    let p = plan(&[&f], &tree);
    assert!(p.is_complete(), "{:?}", p.refused);
    match &p.edits[0] {
        Edit::Rewrite { body, .. } => {
            assert!(body.contains("bus-file"));
            assert!(!body.contains("messaging-file-adapter"));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn many_findings_of_one_rule_become_one_plan() {
    // The point of the codemod: 251 findings are one reviewed change, not 251
    // hand edits.
    let fs: Vec<Finding> = (0..251)
        .map(|i| {
            finding(
                &format!("cap{i}/core/x/Cargo.toml"),
                Some(Fix::RetargetDependency {
                    from: "data/core/types".into(),
                    to: "data/ports/types".into(),
                }),
            )
        })
        .collect();
    let tree: BTreeMap<String, String> = (0..251)
        .map(|i| {
            (
                format!("cap{i}/core/x/Cargo.toml"),
                "dep = \"data/core/types\"\n".to_string(),
            )
        })
        .collect();
    let refs: Vec<&Finding> = fs.iter().collect();
    let p = plan(&refs, &tree);
    assert_eq!(p.edits.len(), 251);
    assert!(p.is_complete(), "{} refused", p.refused.len());
}

#[test]
fn a_finding_with_no_fix_is_reported_as_needing_judgement() {
    // Most findings need a human. That is expected, and must be counted
    // separately from a fix that FAILED, or a caller reads one as the other.
    let f = finding("iam/ports/x/Cargo.toml", None);
    let p = plan(&[&f], &files(&[]));
    assert_eq!(p.needs_judgement(), 1);
    assert_eq!(p.failed(), 0);
    assert!(!p.is_complete(), "nothing was applied");
}

#[test]
fn a_stale_fix_is_refused_not_silently_skipped() {
    // The tree moved since the finding was computed. Applying nothing here is
    // correct; applying it silently and reporting success is not.
    let f = finding(
        "a/core/x/Cargo.toml",
        Some(Fix::RenameSymbol {
            from: "old-name".into(),
            to: "new-name".into(),
        }),
    );
    let tree = files(&[("a/core/x/Cargo.toml", "name = \"something-else\"\n")]);
    let p = plan(&[&f], &tree);
    assert_eq!(p.edits.len(), 0);
    assert!(matches!(
        p.refused.first(),
        Some(Refused::AnchorNotFound { .. })
    ));
    assert_eq!(p.failed(), 1, "a stale anchor is a failure, not judgement");
}

#[test]
fn a_fix_naming_an_absent_file_is_refused() {
    let f = finding(
        "gone/core/x/Cargo.toml",
        Some(Fix::RenameSymbol {
            from: "a".into(),
            to: "b".into(),
        }),
    );
    let p = plan(&[&f], &files(&[]));
    assert!(matches!(
        p.refused.first(),
        Some(Refused::SubjectAbsent { .. })
    ));
}

#[test]
fn a_file_whose_fixes_disagree_takes_none_of_them() {
    // A partially-migrated file is worse than an untouched one: the next run
    // sees a tree in neither state.
    let a = finding(
        "x/core/c/Cargo.toml",
        Some(Fix::RenameSymbol {
            from: "name".into(),
            to: "renamed".into(),
        }),
    );
    let b = finding(
        "x/core/c/Cargo.toml",
        Some(Fix::MovePath {
            from: "x/core/c/Cargo.toml".into(),
            to: "x/ports/c/Cargo.toml".into(),
        }),
    );
    let tree = files(&[("x/core/c/Cargo.toml", "name = \"c\"\n")]);
    let p = plan(&[&a, &b], &tree);
    assert!(p.edits.is_empty(), "no partial application");
    assert!(matches!(p.refused.first(), Some(Refused::Conflict { .. })));
}

#[test]
fn two_moves_of_one_file_are_refused() {
    let a = finding(
        "y/core/c/lib.rs",
        Some(Fix::MovePath {
            from: "y/core/c/lib.rs".into(),
            to: "y/ports/c/lib.rs".into(),
        }),
    );
    let b = finding(
        "y/core/c/lib.rs",
        Some(Fix::MovePath {
            from: "y/core/c/lib.rs".into(),
            to: "y/adapters/c/lib.rs".into(),
        }),
    );
    let p = plan(&[&a, &b], &files(&[("y/core/c/lib.rs", "")]));
    assert!(p.edits.is_empty());
    assert!(matches!(p.refused.first(), Some(Refused::Conflict { .. })));
}

#[test]
fn creating_over_an_existing_file_is_refused() {
    let f = finding(
        "z/core/c/OWNERS",
        Some(Fix::CreatePath {
            path: "z/core/c/OWNERS".into(),
            template: "team\n".into(),
        }),
    );
    let p = plan(&[&f], &files(&[("z/core/c/OWNERS", "someone-else\n")]));
    assert!(matches!(
        p.refused.first(),
        Some(Refused::WouldOverwrite { .. })
    ));
}

#[test]
fn a_scaffold_creates_what_is_missing() {
    let f = finding(
        "z/core/c/OWNERS",
        Some(Fix::CreatePath {
            path: "z/core/c/OWNERS".into(),
            template: "team\n".into(),
        }),
    );
    let p = plan(&[&f], &files(&[]));
    assert!(p.is_complete());
    assert!(matches!(&p.edits[0], Edit::Create { .. }));
}

#[test]
fn planning_writes_nothing_so_a_dry_run_is_the_real_run() {
    // Not a separate code path that can drift from the one that applies.
    let before = files(&[("a/core/x/Cargo.toml", "name = \"old\"\n")]);
    let f = finding(
        "a/core/x/Cargo.toml",
        Some(Fix::RenameSymbol {
            from: "old".into(),
            to: "new".into(),
        }),
    );
    let _ = plan(&[&f], &before);
    assert_eq!(
        before.get("a/core/x/Cargo.toml").unwrap(),
        "name = \"old\"\n"
    );
}
