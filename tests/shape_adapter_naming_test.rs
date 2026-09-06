//! `adapter_not_port_plus_technology`, seeded both ways.
//!
//! The rule reads the tenant's own template out of `naming.adapter_name` and
//! the port names out of the crates the unit declares under its ports face.
//! So the two things that must be shown are that a real defect is reported
//! with the rename that closes it, and that the rule stays silent — as not
//! measured, never as a clean pass — wherever it cannot see a port to compare
//! against.

use anvil::shape::adapters::InMemoryTree;
use anvil::shape::core::adapter_naming::{RULE, template_separator};
use anvil::shape::core::spec::{RuleConfig, RuleMode};
use anvil::shape::core::{DepGraph, Finding, Fix, RuleId, ShapeSpec, SpecSource, measure, resolve};
use std::path::PathBuf;

fn spec(name: &str) -> ShapeSpec {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/shape")
        .join(name);
    ShapeSpec::parse(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn registry() -> serde_json::Value {
    serde_json::json!({
        "capabilities": [ { "name": "iam" }, { "name": "billing" } ],
        "meta_directories": [], "faces": []
    })
}

fn cargo(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\n")
}

fn report_for(tree: &InMemoryTree, spec: &ShapeSpec) -> anvil::shape::core::ShapeReport {
    let resolved = resolve(spec, Some(&registry())).unwrap();
    measure(
        &resolved,
        tree,
        "fixture",
        SpecSource::Proposed("fixture".into()),
        &DepGraph::default(),
    )
}

fn findings_for(tree: &InMemoryTree) -> Vec<Finding> {
    report_for(tree, &spec("oyatie.shape.json"))
        .findings
        .into_iter()
        .filter(|f| f.rule == RuleId::new(RULE))
        .collect()
}

fn why_not_measured(tree: &InMemoryTree, spec: &ShapeSpec) -> Vec<String> {
    report_for(tree, spec)
        .not_measured
        .into_iter()
        .filter(|(r, _)| *r == RuleId::new(RULE))
        .map(|(_, why)| why)
        .collect()
}

/// A unit that declares a port, so the port half of the template is knowable.
fn with_a_port() -> InMemoryTree {
    InMemoryTree::from_paths("fx", &[]).with_file("iam/ports/api/Cargo.toml", &cargo("oya-iam-api"))
}

#[test]
fn adapter_naming_fires_on_an_adapter_that_names_no_port_of_its_unit() {
    let tree =
        with_a_port().with_file("iam/adapters/redis/Cargo.toml", &cargo("oya-redis-adapter"));
    let f = findings_for(&tree);
    assert_eq!(
        f.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(),
        vec!["oya-redis-adapter"],
        "an adapter naming a technology and no port is the defect this rule exists for: {f:?}"
    );
    assert_eq!(f[0].unit.as_deref(), Some("iam"));
    assert!(
        f[0].detail.contains("<port>-<technology>") && f[0].detail.contains("iam"),
        "the detail names the tenant's template and the ports it could have used: {}",
        f[0].detail
    );
    assert_eq!(
        f[0].fix,
        Some(Fix::Rename {
            from: "oya-redis-adapter".into(),
            to: "oya-iam-redis-adapter".into()
        }),
        "one candidate port is one unambiguous rename"
    );
}

#[test]
fn adapter_naming_spares_an_adapter_named_for_its_port_and_a_technology() {
    let tree = with_a_port().with_file("iam/adapters/pg/Cargo.toml", &cargo("oya-iam-pg-adapter"));
    assert!(
        findings_for(&tree).is_empty(),
        "`iam` + `pg` is exactly <port>-<technology>; flagging it would make the rule an \
         assertion that no adapter can satisfy"
    );
    assert!(
        why_not_measured(&tree, &spec("oyatie.shape.json")).is_empty(),
        "the rule saw its subject, so it must not also claim it could not"
    );
}

#[test]
fn an_adapter_that_names_its_port_and_no_technology_is_reported_without_a_rename() {
    let tree = with_a_port().with_file("iam/adapters/x/Cargo.toml", &cargo("oya-iam-adapter"));
    let f = findings_for(&tree);
    assert_eq!(f.len(), 1, "{f:?}");
    assert!(
        f[0].detail.contains("no technology"),
        "the half that is missing is named: {}",
        f[0].detail
    );
    assert_eq!(
        f[0].fix, None,
        "no technology can be invented on the author's behalf"
    );
}

/// I1, the direction that is easier to get wrong: a scan that cannot see its
/// subject must not accuse it.
#[test]
fn a_unit_whose_ports_face_is_empty_is_not_measured_rather_than_accused() {
    let tree = with_a_port().with_file(
        "billing/adapters/pg/Cargo.toml",
        &cargo("oya-billing-pg-adapter"),
    );
    assert!(
        findings_for(&tree).is_empty(),
        "billing declares no port, so nothing is known about what its adapter should be named"
    );
    let why = why_not_measured(&tree, &spec("oyatie.shape.json"));
    assert_eq!(why.len(), 1, "{why:?}");
    assert!(why[0].contains("billing"), "{}", why[0]);
}

/// I1, the other direction: a declared rule with no template is not a pass.
#[test]
fn declaring_the_rule_without_a_template_measures_nothing_and_says_so() {
    let mut untemplated = spec("anvil.shape.json");
    assert_eq!(
        untemplated.naming.adapter_name, None,
        "this fixture is the no-template case; pick another if it grows one"
    );
    untemplated.rules.insert(
        RULE.to_string(),
        RuleConfig {
            mode: RuleMode::BaselineBlockOnNew,
            frozen_empty: false,
        },
    );
    let tree = InMemoryTree::from_paths("fx", &["src/shape/mod.rs"]);
    let why = why_not_measured(&tree, &untemplated);
    assert_eq!(why.len(), 1, "{why:?}");
    assert!(why[0].contains("adapter_name"), "{}", why[0]);
    assert!(
        report_for(&tree, &untemplated)
            .findings
            .iter()
            .all(|f| f.rule != RuleId::new(RULE))
    );
}

#[test]
fn a_tree_with_no_manifest_under_a_face_is_not_measured() {
    let tree = InMemoryTree::from_paths("fx", &["iam/ports/api/src/lib.rs"]);
    let why = why_not_measured(&tree, &spec("oyatie.shape.json"));
    assert_eq!(why.len(), 1, "{why:?}");
    assert!(why[0].contains("loaded"), "{}", why[0]);
}

#[test]
fn a_template_that_names_fewer_than_two_components_is_refused_at_adoption() {
    assert_eq!(
        template_separator("<port>-<technology>").as_deref(),
        Some("-")
    );
    assert_eq!(
        template_separator("<port>::<technology>").as_deref(),
        Some("::")
    );
    for bad in ["<port>", "<port><technology>", "port-technology", ""] {
        assert_eq!(
            template_separator(bad),
            None,
            "{bad:?} names no two components"
        );
    }
    let mut broken = spec("oyatie.shape.json");
    broken.naming.adapter_name = Some("<port>".to_string());
    let problems = anvil::shape::core::validate(&broken);
    assert!(
        problems.iter().any(|p| p.contains("adapter_name")),
        "a template the rule cannot read must be a spec problem, not a silent no-op: {problems:?}"
    );
}
