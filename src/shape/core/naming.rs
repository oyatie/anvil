//! Crate naming rules over Cargo manifests: prefix, layer suffix, and the
//! agreement between a crate's suffix and the face directory it lives in.
//! The manifest reader is deliberately minimal (`[package] name = "..."`).

use super::report::{Finding, Fix, RuleId};
use super::resolve::{ResolvedSpec, ResolvedUnit};
use super::tree::TreeSource;
use crate::shape::core::profile::LanguageProfile;

/// `[package] name` from a Cargo manifest, or `None` (virtual manifest or
/// unparseable).
pub fn package_name(manifest: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(manifest).ok()?;
    let mut in_package = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_package = t == "[package]";
            continue;
        }
        if in_package
            && let Some(rest) = t.strip_prefix("name")
            && let Some(v) = rest.trim_start().strip_prefix('=')
        {
            return Some(v.trim().trim_matches('"').to_string());
        }
    }
    None
}

/// The unit and face directory a manifest path falls under, if any.
fn face_of(
    spec: &ResolvedSpec,
    units: &[ResolvedUnit],
    manifest_path: &str,
) -> Option<(String, String)> {
    let unit = units
        .iter()
        .filter(|u| manifest_path.starts_with(u.root.as_str()))
        .max_by_key(|u| u.root.len())?;
    let skel = spec.spec.skeletons.get(&unit.skeleton)?;
    let rest = &manifest_path[unit.root.len()..];
    skel.faces
        .iter()
        .find(|(_, dir)| rest.starts_with(dir.as_str()))
        .map(|(face, _)| (unit.name.clone(), face.clone()))
}

pub fn naming_findings(
    spec: &ResolvedSpec,
    tree: &dyn TreeSource,
    units: &[ResolvedUnit],
    declared: &dyn Fn(&str) -> bool,
) -> Vec<Finding> {
    let naming = &spec.spec.naming;
    let marker = LanguageProfile::RustCargo.unit_marker();
    let mut out = Vec::new();
    for (path, bytes) in tree.loaded() {
        if path.rsplit('/').next() != Some(marker) {
            continue;
        }
        let Some(name) = package_name(bytes) else {
            continue;
        };
        let at = face_of(spec, units, path);
        let unit_name = at.as_ref().map(|(u, _)| u.clone());

        if declared("crate_name_prefix")
            && !naming.crate_prefix.is_empty()
            && !name.starts_with(naming.crate_prefix.as_str())
        {
            out.push(Finding {
                rule: RuleId::new("crate_name_prefix"),
                key: name.clone(),
                path: path.clone(),
                unit: unit_name.clone(),
                detail: format!(
                    "crate {name} lacks the required prefix {:?}",
                    naming.crate_prefix
                ),
                fix: Some(Fix::Rename {
                    from: name.clone(),
                    to: format!("{}{name}", naming.crate_prefix),
                }),
            });
        }

        if naming.layer_suffixes.is_empty() {
            continue;
        }
        let suffix = name.rsplit('-').next().unwrap_or("").to_string();
        let known = naming.layer_suffixes.contains(&suffix);
        if declared("crate_layer_suffix") && !known {
            out.push(Finding {
                rule: RuleId::new("crate_layer_suffix"),
                key: name.clone(),
                path: path.clone(),
                unit: unit_name.clone(),
                detail: format!(
                    "crate {name} ends in {suffix:?}, not a layer suffix ({})",
                    naming.layer_suffixes.join("|")
                ),
                fix: None,
            });
            continue;
        }
        if declared("suffix_face_disagree")
            && let (Some((_, face)), Some(expected)) = (&at, naming.face_by_suffix.get(&suffix))
            && expected != face
        {
            out.push(Finding {
                rule: RuleId::new("suffix_face_disagree"),
                key: name.clone(),
                path: path.clone(),
                unit: unit_name.clone(),
                detail: format!(
                    "crate {name} has suffix {suffix:?} (face {expected}) but lives under the {face} face"
                ),
                fix: None,
            });
        }
    }
    out
}
