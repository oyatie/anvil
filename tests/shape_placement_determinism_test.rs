//! Placement is a pure function of the spec and the tree: the same inputs in
//! any order give the same report, and every undecidable case names the
//! missing fact instead of guessing.

use anvil::shape::adapters::InMemoryTree;
use anvil::shape::core::{
    DepFacts, DepGraph, PathFacts, Placement, ResolvedSpec, RoleFacts, ShapeSpec, SpecSource,
    measure, place, resolve,
};
use std::path::PathBuf;

fn resolved() -> ResolvedSpec {
    let p =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shape/oyatie.shape.json");
    let spec = ShapeSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap();
    let registry = serde_json::json!({
        "capabilities": [ { "name": "iam" }, { "name": "billing" } ],
        "meta_directories": [ { "dir": "kernel" } ],
        "faces": [ { "face": "core" } ]
    });
    resolve(&spec, Some(&registry)).unwrap()
}

const PATHS: &[&str] = &[
    "iam/core/k/Cargo.toml",
    "iam/observability/slos/a.openslo.yaml",
    "iam/policies/p.json",
    "iam/weird/x.rs",
    "billing/runbooks/r.md",
    "billing/NOTES",
    "oya/legacy/x.rs",
    "kernel/k.rs",
    "scratch.txt",
    "ADR-0001-thing.md",
];

#[test]
fn the_same_tree_in_any_order_yields_an_identical_report() {
    let spec = resolved();
    let forward = InMemoryTree::from_paths("r1", PATHS);
    let mut rev: Vec<&str> = PATHS.to_vec();
    rev.reverse();
    let backward = InMemoryTree::from_paths("r1", &rev);
    let a = measure(
        &spec,
        &forward,
        "fx",
        SpecSource::Adopted,
        &DepGraph::default(),
    );
    let b = measure(
        &spec,
        &backward,
        "fx",
        SpecSource::Adopted,
        &DepGraph::default(),
    );
    assert_eq!(a, b);
    let c = measure(
        &spec,
        &forward,
        "fx",
        SpecSource::Adopted,
        &DepGraph::default(),
    );
    assert_eq!(a, c, "repeated runs are identical");
}

#[test]
fn every_canonical_destination_stays_inside_the_unit_root() {
    let spec = resolved();
    let iam = spec.units.iter().find(|u| u.name == "iam").unwrap().clone();
    for rel in [
        "iam/observability/slos/a.openslo.yaml",
        "iam/policies/p.json",
        "iam/testing/t.md",
    ] {
        let p = PathFacts {
            rel: rel.into(),
            is_dir: false,
        };
        let r = RoleFacts {
            unit: Some(iam.clone()),
            ..Default::default()
        };
        match place(&spec, &p, &r, &DepFacts::default()) {
            Placement::Canonical { dest, .. } => {
                assert!(dest.starts_with("iam/"), "{rel} -> {dest}")
            }
            other => panic!("{rel}: expected Canonical, got {other:?}"),
        }
    }
}

#[test]
fn an_artifact_class_step_places_by_pattern_before_unit_rules() {
    let spec = resolved();
    let p = PathFacts {
        rel: "iam/ADR-0009-x.md".into(),
        is_dir: false,
    };
    let iam = spec.units.iter().find(|u| u.name == "iam").unwrap().clone();
    let r = RoleFacts {
        unit: Some(iam),
        ..Default::default()
    };
    match place(&spec, &p, &r, &DepFacts::default()) {
        Placement::Canonical { dest, step } => {
            assert_eq!(dest, "governance/ADR-0009-x.md");
            assert_eq!(step, "artifact_class:decision");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn crate_level_placement_without_dependency_facts_is_not_measured_not_guessed() {
    let spec = resolved();
    let p = PathFacts {
        rel: "libs/shared-thing/".into(),
        is_dir: true,
    };
    match place(&spec, &p, &RoleFacts::default(), &DepFacts::default()) {
        Placement::NotMeasured { reason } => {
            assert!(reason.contains("dependency facts unavailable"), "{reason}")
        }
        other => panic!("must not guess a crate's home without consumer facts: {other:?}"),
    }
}

#[test]
fn a_path_in_no_unit_is_ambiguous_with_the_reason() {
    let spec = resolved();
    let p = PathFacts {
        rel: "mystery/thing.rs".into(),
        is_dir: false,
    };
    match place(&spec, &p, &RoleFacts::default(), &DepFacts::default()) {
        Placement::Ambiguous { reason, .. } => assert!(reason.contains("no unit"), "{reason}"),
        other => panic!("{other:?}"),
    }
}
