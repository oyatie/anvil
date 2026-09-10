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

fn tenant_of(spec_path: &Path) -> String {
    spec_path
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.split('.').next())
        .expect("a tenant name")
        .to_string()
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
        let tenant = tenant_of(&spec_path);

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

        // Per KIND, not just in total. A `meta` kind shipped here enrolling
        // nothing, and the spec still resolved 21 units, so no test noticed --
        // the same "reads as though it had been checked" shape this file
        // exists to close, one level down. `discover:` kinds enrol at measure
        // time against a tree and cannot resolve here; registry-backed ones
        // can and must.
        let resolved_kinds: std::collections::BTreeSet<&str> =
            resolved.units.iter().map(|u| u.kind.as_str()).collect();
        for (name, kind) in &spec.unit_kinds {
            if kind.members.starts_with("discover:") {
                continue;
            }
            assert!(
                resolved_kinds.contains(name.as_str()),
                "{} declares unit kind {name:?}, which enrols from the registry and \
                 resolves nothing. A kind with no members produces no findings and no \
                 failures, and reads exactly like one that found nothing wrong",
                spec_path.display()
            );
        }
    }
}

/// Tenants whose declared registry location the tenant does not admit yet, and
/// the tenant change each one needs.
///
/// Three rounds put oyatie's registry in three inadmissible places:
/// `governance/` (a `FORBIDDEN_NAMES` root), `.anvil/` (absent from
/// `ALLOWED_DOT_ROOT_DIRS`), and `.config/anvil/` (admitted ROOT, but
/// `layout/root_meta.rs:44-50` matches exactly `[".config", "nextest.toml"]`).
/// Each time the check here was a lexical rule invented in this file, and each
/// time it agreed with the guess.
///
/// Anvil cannot decide admissibility. The predicate is a few lines of code
/// inside the tenant, dispatched per root, and unguessable from outside. So
/// this stops guessing: a declared location that the tenant does not admit is
/// named here with the change that would admit it, which is a diff a reviewer
/// sees and a question its owner can answer.
const PATH_NOT_YET_ADMITTED: &[(&str, &str)] = &[(
    "oyatie",
    "`.config/anvil/` needs validate_config_path (pipeline/core/admission/src/layout/\
     root_meta.rs:44-50) to admit more than the nextest profile. All four of oyatie's \
     dot-roots carry closed schemas and no root is blessed for tool config, so which \
     one opens is a ruling rather than a path to pick",
)];

/// Shipped specs whose marker has NOT been demonstrated against their tenant.
///
/// Not an excuse list -- a visible one. `console.shape.json` discovers on
/// `manifest.json` at `<name>/`, and console's 35 files with that BASENAME sit
/// at depths 4 and 6 (`backend/crates/<x>/<face>/openapi/manifest.json`),
/// never at a unit root, so it discovers zero units and reports a clean zero.
/// (51 paths merely END with the string, across eight basenames -- the suffix
/// error this file exists to catch, committed while describing it.) Measured
/// at console@83b92700 on 2026-09-10. Adding a spec here is a diff a reviewer
/// sees; shipping one silently is what this file exists to stop.
const MARKER_NOT_DEMONSTRATED: &[&str] = &["console"];

/// Shipped specs whose marker IS exercised against a tree shaped like its
/// tenant. Both lists are explicit so that either way of adding a spec is a
/// diff somebody reads.
const MARKER_DEMONSTRATED: &[&str] = &["oyatie", "anvil"];

/// A declared registry location is either one the tenant admits, or one named
/// as not-yet-admitted with the change it needs.
#[test]
fn every_declared_registry_lives_where_anvil_keeps_its_config() {
    for spec_path in shipped_specs() {
        let spec = read_spec(&spec_path);
        let Some(registry_ref) = spec.unit_registry.clone() else {
            continue;
        };
        // A path that climbs can name anything the tenant forbids, whatever
        // its prefix.
        let p = std::path::Path::new(&registry_ref.path);
        assert!(
            p.is_relative()
                && !p
                    .components()
                    .any(|c| matches!(c, std::path::Component::ParentDir)),
            "{} declares its unit registry at {:?}, which escapes upward or is absolute",
            spec_path.display(),
            registry_ref.path
        );
        let tenant = tenant_of(&spec_path);
        assert!(
            PATH_NOT_YET_ADMITTED.iter().any(|(t, _)| *t == tenant),
            "{} declares a unit registry at {:?} and {tenant:?} is not named in \
             PATH_NOT_YET_ADMITTED. Anvil cannot tell whether a tenant admits a \
             directory -- that predicate is code inside the tenant -- so a declared \
             location is either already admitted there or named here with the change \
             it needs",
            spec_path.display(),
            registry_ref.path
        );
    }
}

/// Every shipped spec either demonstrates its marker or is listed as not
/// having done so.
#[test]
fn a_spec_whose_marker_is_undemonstrated_is_named_rather_than_quiet() {
    for spec_path in shipped_specs() {
        let tenant = tenant_of(&spec_path);
        if MARKER_NOT_DEMONSTRATED.contains(&tenant.as_str()) {
            continue;
        }
        // On the FILE NAME. The earlier spelling asked whether the absolute
        // path contained "anvil", and the repository directory is named anvil
        // -- so every spec was "demonstrated" in every real checkout and in
        // CI. The check passed while measuring nothing.
        let demonstrated = MARKER_DEMONSTRATED.contains(&tenant.as_str());
        assert!(
            demonstrated,
            "{} ships a unit marker that no test exercises against a tree shaped like \
             its tenant, and is not named in MARKER_NOT_DEMONSTRATED. A marker that \
             matches nothing discovers no units and reports a clean zero, which reads \
             exactly like conformance",
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
