//! The measurement: runs every rule the spec declares over a tree and
//! assembles the report. Rules this build cannot evaluate are listed as
//! not measured with the reason — they are never counted as clean (I1).

use super::dependency::{DepGraph, dependency_findings, port_findings};
use super::naming::naming_findings;
use super::report::{RuleId, ShapeReport, SpecSource};
use super::resolve::ResolvedSpec;
use super::root_hygiene::root_findings;
use super::skeleton::{discover_units, unit_conformance};
use super::tree::TreeSource;
use crate::shape::core::profile::LanguageProfile;

/// Rules that need dependency edges.
const DEPENDENCY_RULES: &[&str] = &["face_edge_denied", "cross_unit_non_facade"];

const NAMING_RULES: &[&str] = &[
    "crate_name_prefix",
    "crate_layer_suffix",
    "suffix_face_disagree",
];

pub fn measure(
    spec: &ResolvedSpec,
    tree: &dyn TreeSource,
    repo: &str,
    spec_source: SpecSource,
    deps: &DepGraph,
) -> ShapeReport {
    let declared = |rule: &str| spec.spec.rules.contains_key(rule);
    let mut findings = Vec::new();
    let mut not_measured = Vec::new();

    let units = discover_units(spec, tree);
    let mut conformance = Vec::with_capacity(units.len());
    for unit in &units {
        let (c, f) = unit_conformance(spec, tree, unit, &declared);
        conformance.push(c);
        findings.extend(f);
    }

    findings.extend(root_findings(spec, tree, &declared));

    let cargo_declared = spec.spec.profiles.contains(&LanguageProfile::RustCargo);
    let cargo_marker = LanguageProfile::RustCargo.unit_marker();
    let any_manifest = tree
        .loaded()
        .keys()
        .any(|p| p.rsplit('/').next() == Some(cargo_marker));
    if cargo_declared && any_manifest {
        findings.extend(naming_findings(spec, tree, &units, &declared));
    } else {
        for rule in NAMING_RULES.iter().filter(|r| declared(r)) {
            not_measured.push((
                RuleId::new(rule),
                if cargo_declared {
                    format!("no {cargo_marker} loaded from the tree")
                } else {
                    "naming rules need the rust-cargo profile".to_string()
                },
            ));
        }
    }

    if deps.unavailable.is_empty() {
        findings.extend(dependency_findings(spec, &units, deps, &declared));
    } else {
        let why = deps
            .unavailable
            .iter()
            .map(|(p, e)| format!("{p}: {e}"))
            .collect::<Vec<_>>()
            .join("; ");
        for rule in DEPENDENCY_RULES.iter().filter(|r| declared(r)) {
            not_measured.push((RuleId::new(rule), why.clone()));
        }
    }
    if declared("port_defined_in_adapter") {
        let any_rs = tree.loaded().keys().any(|p| p.ends_with(".rs"));
        if any_rs {
            findings.extend(port_findings(spec, &units, tree, &declared));
        } else {
            not_measured.push((
                RuleId::new("port_defined_in_adapter"),
                "no source files loaded under an adapters face".to_string(),
            ));
        }
    }
    if declared("adapter_not_port_plus_technology") {
        not_measured.push((
            RuleId::new("adapter_not_port_plus_technology"),
            "adapter naming check is not available in this build".to_string(),
        ));
    }

    findings.sort_by(|a, b| (&a.rule, &a.key).cmp(&(&b.rule, &b.key)));
    findings.dedup_by(|a, b| a.rule == b.rule && a.key == b.key);

    ShapeReport {
        repo: repo.to_string(),
        rev: tree.rev().to_string(),
        spec_source,
        units: conformance,
        findings,
        not_measured,
    }
}
