//! A tenant proposal that cannot resolve has never been measured.
//!
//! ADR-0006 ships each tenant's spec as a proposal in `tests/fixtures/shape/`
//! until that tenant adopts its own. A proposal is only worth shipping if it
//! can produce a number against the tenant it names, and the way it silently
//! cannot is a discriminator that matches nothing: discovery walks the tree,
//! finds no unit, and every rule reports zero findings over zero units, which
//! reads exactly like conformance.
//!
//! `oyatie.shape.json` shipped that way for as long as it has existed. Its
//! `unit_marker` was `manifest.json`, which no capability or product root
//! carries, and it enumerated capabilities from a
//! `governance/capability-registry.json` that cannot exist -- `governance` is
//! a forbidden root there. It has since stopped naming a registry at all: the
//! capability set is derived from the tree, because a list of what the tree
//! already says is a second source of truth that drifts and cannot be checked
//! against anything.

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

/// EVERY discovering kind finds something in a tree shaped like the tenant.
///
/// Per kind, not in total, and not by naming one kind: a spec with two
/// discovering kinds where only one works would pass a check that names the
/// working one, and the broken one would report a clean zero. That is the
/// console failure in different clothing.
#[test]
fn every_discovering_kind_finds_units_in_an_oyatie_shaped_tree() {
    let spec = read_spec(&fixture_dir().join("oyatie.shape.json"));
    let resolved = resolve(&spec, None).expect("the oyatie proposal resolves without a registry");
    let report = measure(
        &resolved,
        &oyatie_shaped_tree(),
        "oyatie/oyatie",
        SpecSource::Proposed("tests/fixtures/shape/oyatie.shape.json".to_string()),
        &DepGraph::default(),
    );

    let discovering: Vec<&String> = resolved.discovery.iter().map(|d| &d.kind).collect();
    assert!(
        !discovering.is_empty(),
        "the proposal declares no discovering kind, so this proves nothing"
    );
    for kind in discovering {
        let found: Vec<&str> = report
            .units
            .iter()
            .filter(|u| &u.kind == kind)
            .map(|u| u.unit.as_str())
            .collect();
        assert!(
            !found.is_empty(),
            "unit kind {kind:?} discovered nothing in a tree shaped the way oyatie is \
             shaped, so its discriminator names something oyatie does not have and every \
             rule would report zero findings over zero units"
        );
    }
}
