//! Units and their skeleton conformance: which units exist in the tree, which
//! faces each carries, which satellites sit under an alias, and which files
//! and directories belong to no declared place.

use super::placement::{DepFacts, PathFacts, Placement, RoleFacts, place};
use super::report::{Finding, Fix, RuleId, UnitConformance};
use super::resolve::{ResolvedSpec, ResolvedUnit};
use super::tree::TreeSource;
use std::collections::BTreeSet;

/// Registry units plus units discovered by marker file, sorted by root.
pub fn discover_units(spec: &ResolvedSpec, tree: &dyn TreeSource) -> Vec<ResolvedUnit> {
    let mut units: Vec<ResolvedUnit> = spec.units.clone();
    for rule in &spec.discovery {
        let Some((prefix, suffix)) = rule.root_pattern.split_once("<name>") else {
            continue;
        };
        let mut seen = BTreeSet::new();
        for path in tree.under(prefix).iter() {
            let rest = &path[prefix.len()..];
            let Some(name) = rest.split('/').next() else {
                continue;
            };
            if name.is_empty() || !seen.insert(name.to_string()) {
                continue;
            }
            let root = format!("{prefix}{name}{suffix}");
            if tree.contains(&format!("{root}{}", rule.marker)) {
                let over = spec.spec.units.get(name);
                units.push(ResolvedUnit {
                    name: name.to_string(),
                    kind: rule.kind.clone(),
                    root,
                    skeleton: rule.skeleton.clone(),
                    destination_stable: over
                        .and_then(|o| o.destination_stable)
                        .unwrap_or(spec.spec.destination_stable_default),
                    satellites_not_applicable: over
                        .map(|o| o.satellites_not_applicable.clone())
                        .unwrap_or_default(),
                });
            }
        }
    }
    units.sort_by(|a, b| a.root.cmp(&b.root));
    units.dedup_by(|a, b| a.root == b.root);
    units
}

/// One unit's conformance and the findings that describe its distance.
/// Only rules the spec declares are emitted; the caller filters by `rules`.
pub fn unit_conformance(
    spec: &ResolvedSpec,
    tree: &dyn TreeSource,
    unit: &ResolvedUnit,
    declared: &dyn Fn(&str) -> bool,
) -> (UnitConformance, Vec<Finding>) {
    let mut findings = Vec::new();
    let Some(skel) = spec.spec.skeletons.get(&unit.skeleton) else {
        return (
            UnitConformance {
                unit: unit.name.clone(),
                kind: unit.kind.clone(),
                faces_present: vec![],
                faces_missing: vec![],
                satellites_aliased: vec![],
                destination_stable: unit.destination_stable,
            },
            findings,
        );
    };

    let mut faces_present = Vec::new();
    let mut faces_missing = Vec::new();
    for (face, dir) in &skel.faces {
        if tree.has_dir(&format!("{}{dir}", unit.root)) {
            faces_present.push(face.clone());
        } else if skel.required_faces.iter().any(|f| f == face) {
            faces_missing.push(face.clone());
            if declared("unit_missing_face") {
                findings.push(Finding {
                    rule: RuleId::new("unit_missing_face"),
                    key: format!("{}:{face}", unit.name),
                    path: unit.root.clone(),
                    unit: Some(unit.name.clone()),
                    detail: format!("unit {} has no {face} face ({dir})", unit.name),
                    fix: Some(Fix::Create {
                        path: format!("{}{dir}", unit.root),
                        content: None,
                    }),
                });
            }
        }
    }

    let mut satellites_aliased = BTreeSet::new();
    let mut ambiguous_dirs = BTreeSet::new();
    for path in tree.under(&unit.root).iter() {
        let facts = PathFacts {
            rel: path.to_string(),
            is_dir: false,
        };
        let role = RoleFacts {
            unit: Some(unit.clone()),
            ..Default::default()
        };
        match place(spec, &facts, &role, &DepFacts::default()) {
            Placement::AlreadyCanonical { .. } => {}
            Placement::Canonical { dest, step } => {
                if let Some(class) = step.strip_prefix("satellite:") {
                    satellites_aliased.insert(class.to_string());
                    if declared("satellite_alias_used") {
                        findings.push(Finding {
                            rule: RuleId::new("satellite_alias_used"),
                            key: path.to_string(),
                            path: path.to_string(),
                            unit: Some(unit.name.clone()),
                            detail: format!(
                                "{class} artifact under an alias; canonical home is {dest}"
                            ),
                            fix: Some(Fix::Move {
                                from: path.to_string(),
                                to: dest,
                            }),
                        });
                    }
                } else if declared("file_misplaced") {
                    findings.push(Finding {
                        rule: RuleId::new("file_misplaced"),
                        key: path.to_string(),
                        path: path.to_string(),
                        unit: Some(unit.name.clone()),
                        detail: format!("placed by {step}; canonical home is {dest}"),
                        fix: Some(Fix::Move {
                            from: path.to_string(),
                            to: dest,
                        }),
                    });
                }
            }
            Placement::Ambiguous { reason, .. } => {
                let rest = &path[unit.root.len()..];
                if rest.contains('/') {
                    // One finding per stray directory, not per file in it.
                    let first = rest.split('/').next().unwrap_or(rest);
                    let key = format!("{}{first}/", unit.root);
                    if !ambiguous_dirs.insert(key.clone()) {
                        continue;
                    }
                    if declared("placement_ambiguous") {
                        findings.push(Finding {
                            rule: RuleId::new("placement_ambiguous"),
                            key: key.clone(),
                            path: key,
                            unit: Some(unit.name.clone()),
                            detail: reason,
                            fix: None,
                        });
                    }
                } else if declared("unit_root_file_unallowlisted") {
                    findings.push(Finding {
                        rule: RuleId::new("unit_root_file_unallowlisted"),
                        key: path.to_string(),
                        path: path.to_string(),
                        unit: Some(unit.name.clone()),
                        detail: reason,
                        fix: None,
                    });
                }
            }
            Placement::NotMeasured { .. } => {}
        }
    }

    (
        UnitConformance {
            unit: unit.name.clone(),
            kind: unit.kind.clone(),
            faces_present,
            faces_missing,
            satellites_aliased: satellites_aliased.into_iter().collect(),
            destination_stable: unit.destination_stable,
        },
        findings,
    )
}
