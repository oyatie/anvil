//! Every shape rule must be able to fail (I9), and each seeded defect must be
//! reported under its own rule id (G17) with the fix attached. A rule that
//! cannot fail is an assertion, not a measurement.

use anvil::shape::adapters::InMemoryTree;
use anvil::shape::core::{Fix, RuleId, ShapeSpec, SpecSource, measure, resolve};
use std::path::PathBuf;

fn spec(name: &str) -> ShapeSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/shape")
        .join(name);
    ShapeSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn registry() -> serde_json::Value {
    serde_json::json!({
        "capabilities": [ { "name": "iam" }, { "name": "storage" } ],
        "meta_directories": [ { "dir": "kernel" } ],
        "faces": [ { "face": "core" }, { "face": "ports" }, { "face": "adapters" }, { "face": "facade" } ]
    })
}

fn findings_for(tree: &InMemoryTree, rule: &str) -> Vec<anvil::shape::core::Finding> {
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry())).unwrap();
    let report = measure(
        &resolved,
        tree,
        "fixture",
        SpecSource::Proposed("fixture".into()),
    );
    report
        .findings
        .into_iter()
        .filter(|f| f.rule == RuleId::new(rule))
        .collect()
}

#[test]
fn slo_under_observability_is_an_alias_with_a_move() {
    let tree = InMemoryTree::from_paths(
        "fx",
        &[
            "iam/core/x/Cargo.toml",
            "iam/observability/slos/auth.openslo.yaml",
        ],
    );
    let f = findings_for(&tree, "satellite_alias_used");
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(
        f[0].fix,
        Some(Fix::Move {
            from: "iam/observability/slos/auth.openslo.yaml".into(),
            to: "iam/slos/auth.openslo.yaml".into()
        })
    );
    assert_eq!(f[0].unit.as_deref(), Some("iam"));
}

#[test]
fn policies_is_an_alias_of_policy() {
    let tree = InMemoryTree::from_paths("fx", &["iam/policies/rbac.json"]);
    let f = findings_for(&tree, "satellite_alias_used");
    assert_eq!(f.len(), 1);
    assert!(matches!(&f[0].fix, Some(Fix::Move { to, .. }) if to == "iam/policy/rbac.json"));
}

#[test]
fn a_unit_missing_a_required_face_is_reported_per_face() {
    // `storage` exists with a core face only; `iam` has no directory at all.
    let tree = InMemoryTree::from_paths("fx", &["storage/core/x/Cargo.toml"]);
    let f = findings_for(&tree, "unit_missing_face");
    let keys: Vec<&str> = f.iter().map(|f| f.key.as_str()).collect();
    assert!(
        keys.contains(&"storage:ports") && keys.contains(&"storage:facade"),
        "{keys:?}"
    );
    assert!(!keys.contains(&"storage:core"));
    assert!(
        keys.contains(&"iam:core"),
        "a registered unit with no directory misses every face: {keys:?}"
    );
    assert!(f.iter().all(|f| matches!(f.fix, Some(Fix::Create { .. }))));
}

#[test]
fn a_stray_directory_inside_a_unit_is_ambiguous_once_not_per_file() {
    let tree = InMemoryTree::from_paths(
        "fx",
        &[
            "iam/oya-identity/src/a.rs",
            "iam/oya-identity/src/b.rs",
            "iam/oya-identity/Cargo.toml",
        ],
    );
    let f = findings_for(&tree, "placement_ambiguous");
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].key, "iam/oya-identity/");
    assert!(
        f[0].detail
            .contains("neither a face nor a declared satellite")
    );
}

#[test]
fn a_unit_root_file_off_the_allowlist_is_reported() {
    let tree = InMemoryTree::from_paths("fx", &["iam/NOTES.txt", "iam/OWNERS"]);
    let f = findings_for(&tree, "unit_root_file_unallowlisted");
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].key, "iam/NOTES.txt");
}

#[test]
fn a_root_file_off_the_allowlist_is_reported_and_readme_variants_pass() {
    let tree =
        InMemoryTree::from_paths("fx", &["scratch.txt", "README.md", "README", "Cargo.toml"]);
    let f = findings_for(&tree, "root_file_unallowlisted");
    assert_eq!(
        f.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        vec!["scratch.txt"]
    );
}

#[test]
fn growth_under_a_legacy_root_is_keyed_per_path() {
    let tree = InMemoryTree::from_paths("fx", &["oya/payments/src/lib.rs", "cloud/x.rs"]);
    let f = findings_for(&tree, "legacy_root_growth");
    assert_eq!(f.len(), 2);
}

#[test]
fn crate_naming_rules_fire_on_prefix_suffix_and_face_disagreement() {
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file(
            "iam/core/thing/Cargo.toml",
            "[package]\nname = \"thing-kernel\"\n",
        )
        .with_file(
            "iam/core/odd/Cargo.toml",
            "[package]\nname = \"oya-iam-widget\"\n",
        )
        .with_file(
            "iam/facade/dom/Cargo.toml",
            "[package]\nname = \"oya-iam-domain\"\n",
        );
    let prefix = findings_for(&tree, "crate_name_prefix");
    assert_eq!(
        prefix.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        vec!["thing-kernel"]
    );
    let suffix = findings_for(&tree, "crate_layer_suffix");
    assert_eq!(
        suffix.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        vec!["oya-iam-widget"]
    );
    let disagree = findings_for(&tree, "suffix_face_disagree");
    assert_eq!(
        disagree.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        vec!["oya-iam-domain"]
    );
    assert!(disagree[0].detail.contains("face core") && disagree[0].detail.contains("facade face"));
}

#[test]
fn hand_listed_workspace_members_are_reported_globs_are_not() {
    let tree = InMemoryTree::from_paths("fx", &[]).with_file(
        "Cargo.toml",
        "[workspace]\nmembers = [\n  \"*/core/*\",\n  \"libs/special-crate\", # reviewed\n]\n",
    );
    let f = findings_for(&tree, "workspace_members_not_glob");
    assert_eq!(
        f.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        vec!["libs/special-crate"]
    );
}

#[test]
fn an_app_is_discovered_by_marker_and_held_to_the_same_skeleton() {
    let tree = InMemoryTree::from_paths(
        "fx",
        &[
            "app/calendar/manifest.json",
            "app/calendar/slos/x.openslo.yaml",
            "app/calendar/observability/slos/y.openslo.yaml",
        ],
    );
    let resolved = resolve(&spec("oyatie.shape.json"), Some(&registry())).unwrap();
    let report = measure(
        &resolved,
        &tree,
        "fixture",
        SpecSource::Proposed("fixture".into()),
    );
    let cal = report
        .units
        .iter()
        .find(|u| u.unit == "calendar")
        .expect("app discovered");
    assert_eq!(cal.kind, "app");
    assert_eq!(cal.faces_missing, vec!["core", "facade", "ports"]);
    assert_eq!(cal.satellites_aliased, vec!["slos"]);
}

#[test]
fn an_undeclared_rule_produces_no_findings() {
    // The console fixture declares no naming rules; the same defect is silent there.
    let tree = InMemoryTree::from_paths("fx", &[])
        .with_file("x/core/thing/Cargo.toml", "[package]\nname = \"thing\"\n");
    let resolved = resolve(&spec("console.shape.json"), None).unwrap();
    let report = measure(
        &resolved,
        &tree,
        "fixture",
        SpecSource::Proposed("fixture".into()),
    );
    assert!(
        report
            .findings
            .iter()
            .all(|f| f.rule != RuleId::new("crate_layer_suffix"))
    );
}
