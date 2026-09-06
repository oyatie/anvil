//! Dependency edges classified by unit and face, and the two rules over
//! them: the Dependency Rule inside a unit (`face_edge_denied`) and the
//! facade-only rule between units (`cross_unit_non_facade`). Plus
//! `port_defined_in_adapter`: a port trait declared under an adapters face.

use super::report::{Finding, Fix, RuleId};
use super::resolve::{ResolvedSpec, ResolvedUnit};
use super::spec::CrossUnitEdges;
use super::tree::TreeSource;
use crate::shape::core::profile::LanguageProfile;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DepEdge {
    /// Repository-relative path of the dependent (a crate dir, BUCK package
    /// dir, or source file).
    pub from: String,
    /// Repository-relative path of the dependency (a crate dir, package dir,
    /// or module dir).
    pub to: String,
    /// How the edge was read: "cargo path", "buck label", "use crate::".
    pub via: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DepGraph {
    pub edges: Vec<DepEdge>,
    /// Profiles whose adapter could not run, with the reason. Any entry makes
    /// the dependency rules NotMeasured: a graph with a missing source is not
    /// a graph with no violations.
    pub unavailable: Vec<(LanguageProfile, String)>,
}

/// (unit, face) of a path; face is None inside a unit but outside every face.
pub fn classify<'a>(
    spec: &ResolvedSpec,
    units: &'a [ResolvedUnit],
    path: &str,
) -> Option<(&'a ResolvedUnit, Option<String>)> {
    let unit = units
        .iter()
        .filter(|u| path.starts_with(u.root.as_str()))
        .max_by_key(|u| u.root.len())?;
    let skel = spec.spec.skeletons.get(&unit.skeleton)?;
    let rest = &path[unit.root.len()..];
    let face = skel
        .faces
        .iter()
        .find(|(_, dir)| rest.starts_with(dir.as_str()))
        .map(|(f, _)| f.clone());
    Some((unit, face))
}

pub fn dependency_findings(
    spec: &ResolvedSpec,
    units: &[ResolvedUnit],
    graph: &DepGraph,
    declared: &dyn Fn(&str) -> bool,
) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for e in &graph.edges {
        let Some((fu, ff)) = classify(spec, units, &e.from) else {
            continue;
        };
        let Some((tu, tf)) = classify(spec, units, &e.to) else {
            continue;
        };
        let Some(skel) = spec.spec.skeletons.get(&fu.skeleton) else {
            continue;
        };
        let key = format!(
            "{}->{}",
            e.from.trim_end_matches('/'),
            e.to.trim_end_matches('/')
        );
        if !seen.insert(key.clone()) {
            continue;
        }
        if fu.root == tu.root {
            if declared("face_edge_denied")
                && let (Some(from_face), Some(to_face)) = (&ff, &tf)
                && from_face != to_face
            {
                let allowed = skel
                    .face_dependency_matrix
                    .get(from_face)
                    .is_some_and(|v| v.contains(to_face));
                if !allowed {
                    // Suggest the allowed face that can itself reach the
                    // denied target (facade -> ports, because ports -> core),
                    // else the first allowed face.
                    let allowed_faces = skel
                        .face_dependency_matrix
                        .get(from_face)
                        .cloned()
                        .unwrap_or_default();
                    let via_face = allowed_faces
                        .iter()
                        .find(|f| {
                            skel.face_dependency_matrix
                                .get(*f)
                                .is_some_and(|v| v.contains(to_face))
                        })
                        .or(allowed_faces.first());
                    let ports_dir = via_face
                        .and_then(|f| skel.faces.get(f))
                        .map(|d| format!("{}{d}", fu.root));
                    out.push(Finding {
                        rule: RuleId::new("face_edge_denied"),
                        key,
                        path: e.from.clone(),
                        unit: Some(fu.name.clone()),
                        detail: format!(
                            "{from_face} may not depend on {to_face} ({}); allowed: {:?}",
                            e.via,
                            skel.face_dependency_matrix
                                .get(from_face)
                                .cloned()
                                .unwrap_or_default()
                        ),
                        fix: ports_dir.map(|with| Fix::DependOnInstead {
                            replace: e.to.clone(),
                            with,
                        }),
                    });
                }
            }
        } else if declared("cross_unit_non_facade")
            && skel.cross_unit_edges == CrossUnitEdges::FacadeOnly
        {
            let Some(tskel) = spec.spec.skeletons.get(&tu.skeleton) else {
                continue;
            };
            let target_is_facade = tf.as_deref().is_some_and(|f| {
                // "facade" is whichever face the target's matrix lets nothing else reach through:
                // the face other units are meant to call. We take the face named "facade" when
                // present, else any face no other face of that skeleton may depend on.
                tskel.faces.contains_key(f)
                    && (f == "facade"
                        || !tskel
                            .face_dependency_matrix
                            .values()
                            .any(|v| v.iter().any(|x| x == f)))
            });
            if !target_is_facade {
                let facade_dir = tskel
                    .faces
                    .get("facade")
                    .map(|d| format!("{}{d}", tu.root))
                    .unwrap_or_else(|| tu.root.clone());
                out.push(Finding {
                    rule: RuleId::new("cross_unit_non_facade"),
                    key,
                    path: e.from.clone(),
                    unit: Some(fu.name.clone()),
                    detail: format!(
                        "{} reaches into {}'s {} ({}); another unit may be reached only through its facade",
                        fu.name,
                        tu.name,
                        tf.as_deref().unwrap_or("unlayered code"),
                        e.via
                    ),
                    fix: Some(Fix::DependOnInstead { replace: e.to.clone(), with: facade_dir }),
                });
            }
        }
    }
    out
}

/// `pub trait <Name>` with a port-name suffix, declared under an adapters face.
pub fn port_findings(
    spec: &ResolvedSpec,
    units: &[ResolvedUnit],
    tree: &dyn TreeSource,
    declared: &dyn Fn(&str) -> bool,
) -> Vec<Finding> {
    let mut out = Vec::new();
    if !declared("port_defined_in_adapter") || spec.spec.naming.port_name_suffixes.is_empty() {
        return out;
    }
    for (path, bytes) in tree.loaded() {
        if !path.ends_with(".rs") {
            continue;
        }
        let Some((unit, Some(face))) = classify(spec, units, path) else {
            continue;
        };
        if face != "adapters" {
            continue;
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            continue;
        };
        for line in text.lines() {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix("pub trait ") else {
                continue;
            };
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if spec
                .spec
                .naming
                .port_name_suffixes
                .iter()
                .any(|s| name.ends_with(s.as_str()))
            {
                out.push(Finding {
                    rule: RuleId::new("port_defined_in_adapter"),
                    key: format!("{path}:{name}"),
                    path: path.clone(),
                    unit: Some(unit.name.clone()),
                    detail: format!("port trait {name} is declared under the adapters face; a port belongs to the ports or core face"),
                    fix: None,
                });
            }
        }
    }
    out
}
