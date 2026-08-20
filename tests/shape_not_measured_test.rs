//! What the engine cannot measure it must say it cannot measure (I1). A rule
//! whose inputs are missing is listed under `not_measured` with the reason;
//! it never appears as zero findings.

use anvil::shape::adapters::InMemoryTree;
use anvil::shape::core::{RuleId, ShapeSpec, SpecSource, measure, resolve};
use std::path::PathBuf;

fn spec(name: &str) -> ShapeSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/shape")
        .join(name);
    ShapeSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn dependency_rules_are_listed_as_not_measured_in_this_build() {
    let resolved = resolve(&spec("anvil.shape.json"), None).unwrap();
    let tree = InMemoryTree::from_paths("fx", &["src/shape/mod.rs"]);
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted);
    let ids: Vec<&str> = report
        .not_measured
        .iter()
        .map(|(r, _)| r.0.as_str())
        .collect();
    assert!(ids.contains(&"face_edge_denied"), "{ids:?}");
    assert!(ids.contains(&"cross_unit_non_facade"), "{ids:?}");
    assert!(
        report
            .not_measured
            .iter()
            .all(|(_, why)| !why.trim().is_empty()),
        "every not-measured entry names a reason"
    );
}

#[test]
fn naming_rules_without_a_manifest_in_the_tree_are_not_measured() {
    let registry = serde_json::json!({ "capabilities": [{"name":"iam"}], "meta_directories": [], "faces": [] });
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry)).unwrap();
    let tree = InMemoryTree::from_paths("fx", &["iam/core/k/src/lib.rs"]); // no Cargo.toml loaded
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted);
    let ids: Vec<&str> = report
        .not_measured
        .iter()
        .map(|(r, _)| r.0.as_str())
        .collect();
    assert!(ids.contains(&"crate_layer_suffix"), "{ids:?}");
    assert!(
        !report
            .findings
            .iter()
            .any(|f| f.rule == RuleId::new("crate_layer_suffix"))
    );
}

#[test]
fn a_proposed_spec_is_stamped_proposed_never_adopted() {
    let resolved = resolve(&spec("anvil.shape.json"), None).unwrap();
    let tree = InMemoryTree::from_paths("fx", &[]);
    let report = measure(
        &resolved,
        &tree,
        "fx",
        SpecSource::Proposed("fixture".into()),
    );
    assert_eq!(report.spec_source, SpecSource::Proposed("fixture".into()));
}

#[test]
fn distance_is_derived_from_findings_not_stored() {
    let resolved = resolve(&spec("anvil.shape.json"), None).unwrap();
    let tree = InMemoryTree::from_paths("fx", &["src/thing/mod.rs", "stray.bin"]);
    let report = measure(&resolved, &tree, "fx", SpecSource::Adopted);
    let d = report.distance();
    assert_eq!(d.findings_total, report.findings.len());
    assert_eq!(d.units_total, report.units.len());
    assert_eq!(d.units_total, 1, "src/thing discovered by mod.rs marker");
}
