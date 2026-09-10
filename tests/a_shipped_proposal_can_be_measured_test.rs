//! A tenant proposal that cannot resolve has never been measured.
//!
//! ADR-0006 ships each tenant's spec as a proposal in `tests/fixtures/shape/`
//! until that tenant adopts its own. A proposal is only worth shipping if it
//! can produce a number against the tenant it names. Two ways it silently
//! cannot:
//!
//! 1. It declares a `unit_registry` and no registry document accompanies it.
//!    `resolve` then refuses -- correctly, per ADR-0006 §4, which prefers
//!    resolving nothing to guessing -- so every rule reports on zero units.
//! 2. Its `unit_marker` names a file the tenant does not put at a unit root.
//!    Discovery walks the tree, matches nothing, and the report is a clean
//!    zero that looks like conformance.
//!
//! Both fail the same way: no error, no findings, and a spec that reads as
//! though it had been checked. `oyatie.shape.json` shipped with both -- a
//! registry at `governance/capability-registry.json` and a `manifest.json`
//! marker, neither of which exists in oyatie.

use anvil::shape::adapters::InMemoryTree;
use anvil::shape::core::{DepGraph, ShapeSpec, SpecSource, measure, resolve};
use std::path::{Path, PathBuf};

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/shape")
}

fn read_spec(path: &Path) -> ShapeSpec {
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    ShapeSpec::parse(&text).unwrap_or_else(|e| panic!("parsing {}: {e:?}", path.display()))
}

fn shipped_specs() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(fixture_dir())
        .expect("the shape fixture directory")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.to_string_lossy().ends_with(".shape.json"))
        .collect();
    found.sort();
    assert!(!found.is_empty(), "no shipped shape proposals were found");
    found
}

/// The registry a spec names must travel with it, and must resolve members.
#[test]
fn every_shipped_proposal_that_names_a_registry_ships_one_that_resolves() {
    for spec_path in shipped_specs() {
        let spec = read_spec(&spec_path);
        let Some(registry_ref) = spec.unit_registry.clone() else {
            continue;
        };
        let tenant = spec_path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.split('.').next())
            .expect("a tenant name")
            .to_string();

        // Named for the tenant, beside the spec, because the tenant's own
        // copy lives at a path only that tenant has.
        let companion = fixture_dir().join(format!("{tenant}.capability-registry.json"));
        assert!(
            companion.exists(),
            "{} declares unit_registry {:?} but no companion registry ships beside it, \
             so `resolve` refuses and every rule reports on zero units",
            spec_path.display(),
            registry_ref.path
        );

        let doc: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&companion).expect("the companion registry"),
        )
        .expect("the companion registry parses as JSON");

        let resolved = resolve(&spec, Some(&doc)).unwrap_or_else(|e| {
            panic!(
                "{} does not resolve against its registry: {e:?}",
                spec_path.display()
            )
        });
        assert!(
            !resolved.units.is_empty(),
            "{} resolved zero units against its own registry, so a measurement \
             against it would report zero findings without measuring anything",
            spec_path.display()
        );
    }
}

/// A tree shaped the way oyatie is actually shaped, measured at b4556b57:
/// 21 capabilities carrying `OWNERS`, faces from core/ports/adapters/facade,
/// and 7 products under `app/<product>/`.
fn oyatie_shaped_tree() -> InMemoryTree {
    let mut paths = vec!["Cargo.toml".to_string()];
    for capability in ["pipeline", "iam", "billing"] {
        paths.push(format!("{capability}/OWNERS"));
        for face in ["core", "ports", "adapters", "facade"] {
            paths.push(format!("{capability}/{face}/a-crate/Cargo.toml"));
        }
    }
    for product in ["payroll", "community"] {
        paths.push(format!("app/{product}/OWNERS"));
        paths.push(format!("app/{product}/core/a-crate/Cargo.toml"));
    }
    let borrowed: Vec<&str> = paths.iter().map(|p| p.as_str()).collect();
    InMemoryTree::from_paths("b4556b57", &borrowed)
}

/// The marker has to match where the tenant actually puts it.
///
/// Discovery forms `root + marker` and asks the tree for it. A marker the
/// tenant does not use at a unit root matches nothing, and the run is a clean
/// zero rather than a refusal -- the failure this file exists to prevent.
#[test]
fn the_oyatie_proposal_discovers_products_in_an_oyatie_shaped_tree() {
    let spec = read_spec(&fixture_dir().join("oyatie.shape.json"));
    let doc: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_dir().join("oyatie.capability-registry.json"))
            .expect("the companion registry"),
    )
    .expect("the companion registry parses");

    let resolved = resolve(&spec, Some(&doc)).expect("the oyatie proposal resolves");
    let tree = oyatie_shaped_tree();
    let report = measure(
        &resolved,
        &tree,
        "oyatie/oyatie",
        SpecSource::Proposed("tests/fixtures/shape/oyatie.shape.json".to_string()),
        &DepGraph::default(),
    );

    let products: Vec<&str> = report
        .units
        .iter()
        .filter(|u| u.kind == "app")
        .map(|u| u.unit.as_str())
        .collect();
    assert!(
        products.contains(&"payroll") && products.contains(&"community"),
        "the app unit kind discovered {products:?} in a tree shaped the way oyatie \
         is shaped, so its marker names a file oyatie does not put at a unit root"
    );
}
