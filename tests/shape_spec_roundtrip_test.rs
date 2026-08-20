//! The shape spec is data a tenant commits; Anvil must read it exactly, reject
//! what it does not understand, and never run a rule the spec did not declare.
//!
//! A spec that round-trips with a silent field drop is how a tenant believes a
//! rule is enforced when it is not — the same failure as a gate that reads
//! its policy file and ignores half of it.

use anvil::shape::core::spec::{MembersSource, RuleMode};
use anvil::shape::core::{resolve, ShapeSpec, SpecError, SCHEMA_V1};
use std::path::PathBuf;

fn fixture(name: &str) -> String {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/shape")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn every_fixture_parses_and_round_trips_losslessly() {
    for name in [
        "oyatie.shape.json",
        "console.shape.json",
        "anvil.shape.json",
    ] {
        let raw = fixture(name);
        let spec = ShapeSpec::parse(&raw).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(spec.schema, SCHEMA_V1);
        let again =
            ShapeSpec::parse(&spec.to_json()).unwrap_or_else(|e| panic!("{name} re-parse: {e}"));
        assert_eq!(
            spec, again,
            "{name} must survive serialize -> parse unchanged"
        );
    }
}

#[test]
fn an_unknown_key_is_rejected_not_ignored() {
    let raw = fixture("anvil.shape.json").replace("\"profiles\"", "\"profilez\": [], \"profiles\"");
    match ShapeSpec::parse(&raw) {
        Err(SpecError::Parse(msg)) => assert!(msg.contains("profilez"), "{msg}"),
        other => panic!("unknown key must be a parse error, got {other:?}"),
    }
}

#[test]
fn semantic_problems_are_all_reported_at_once() {
    let raw = fixture("anvil.shape.json")
        .replace(
            "\"required_faces\": [\"core\", \"ports\", \"facade\"]",
            "\"required_faces\": [\"nope\"]",
        )
        .replace("\"skeleton\": \"standard\"", "\"skeleton\": \"missing\"");
    match ShapeSpec::parse(&raw) {
        Err(SpecError::Invalid(problems)) => {
            assert!(
                problems.iter().any(|p| p.contains("unknown face \"nope\"")),
                "{problems:?}"
            );
            assert!(
                problems
                    .iter()
                    .any(|p| p.contains("unknown skeleton \"missing\"")),
                "{problems:?}"
            );
        }
        other => panic!("expected Invalid with both problems, got {other:?}"),
    }
}

#[test]
fn a_rule_the_spec_does_not_declare_is_not_run() {
    let spec = ShapeSpec::parse(&fixture("anvil.shape.json")).unwrap();
    assert!(spec.rules.contains_key("face_edge_denied"));
    assert!(
        !spec.rules.contains_key("crate_layer_suffix"),
        "fixture premise: the anvil spec leaves crate_layer_suffix undeclared"
    );
    assert_eq!(
        spec.rules["face_edge_denied"].mode,
        RuleMode::BaselineBlockOnNew
    );
    assert_eq!(
        spec.rules["unit_missing_face"].mode,
        RuleMode::AdvisoryUntilInfra
    );
}

#[test]
fn registry_backed_units_resolve_from_the_tenant_registry() {
    let spec = ShapeSpec::parse(&fixture("oyatie.shape.json")).unwrap();
    let registry = serde_json::json!({
        "capabilities": [ { "name": "iam" }, { "name": "storage" } ],
        "meta_directories": [ { "dir": "kernel" }, { "dir": "governance" } ],
        "faces": [ { "face": "core" }, { "face": "ports" }, { "face": "adapters" }, { "face": "facade" } ]
    });
    let resolved = resolve(&spec, Some(&registry)).expect("resolves");
    let names: Vec<&str> = resolved.units.iter().map(|u| u.name.as_str()).collect();
    assert!(
        names.contains(&"iam") && names.contains(&"storage"),
        "{names:?}"
    );
    assert!(
        names.contains(&"kernel") && names.contains(&"governance"),
        "{names:?}"
    );
    let iam = resolved.units.iter().find(|u| u.name == "iam").unwrap();
    assert_eq!(iam.root, "iam/");
    assert_eq!(iam.kind, "capability");
    assert!(
        !iam.destination_stable,
        "destination_stable_default is false"
    );
    assert_eq!(
        resolved.discovery.len(),
        1,
        "app units are discovered by marker, not listed: {:?}",
        resolved.discovery
    );
    assert_eq!(
        resolved.registry_faces.as_deref(),
        Some(
            &[
                "core".to_string(),
                "ports".into(),
                "adapters".into(),
                "facade".into()
            ][..]
        )
    );
}

#[test]
fn a_registry_backed_spec_without_a_registry_document_is_not_guessed() {
    let spec = ShapeSpec::parse(&fixture("oyatie.shape.json")).unwrap();
    match resolve(&spec, None) {
        Err(SpecError::Registry(msg)) => assert!(msg.contains("no registry document"), "{msg}"),
        other => panic!("must refuse to invent units, got {other:?}"),
    }
}

#[test]
fn members_source_grammar_is_exact() {
    let spec = ShapeSpec::parse(&fixture("oyatie.shape.json")).unwrap();
    assert_eq!(
        spec.unit_kinds["capability"].members_source(),
        Ok(MembersSource::Registry)
    );
    assert_eq!(
        spec.unit_kinds["meta"].members_source(),
        Ok(MembersSource::RegistryMetaDirs)
    );
    assert_eq!(
        spec.unit_kinds["app"].members_source(),
        Ok(MembersSource::Discover {
            marker: "manifest.json".into()
        })
    );
}
