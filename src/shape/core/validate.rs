//! Semantic validation of a parsed spec: every problem is reported, none is
//! fatal on its own, so a tenant fixes a spec in one round.

use super::spec::{RuleMode, SCHEMA_V1, ShapeSpec};
use std::collections::{BTreeMap, BTreeSet};

pub fn validate(spec: &ShapeSpec) -> Vec<String> {
    let mut problems = Vec::new();

    if spec.schema != SCHEMA_V1 {
        problems.push(format!(
            "schema must be {SCHEMA_V1:?}, got {:?}",
            spec.schema
        ));
    }
    if spec.profiles.is_empty() {
        problems.push("profiles must name at least one language profile".into());
    }
    if spec.unit_kinds.is_empty() {
        problems.push("unit_kinds must declare at least one unit kind".into());
    }
    if spec.rules.is_empty() {
        problems.push("rules must declare at least one rule; an undeclared rule is not run".into());
    }

    let mut needs_registry = false;
    for (kind_name, kind) in &spec.unit_kinds {
        if !kind.root.contains("<name>") {
            problems.push(format!(
                "unit_kinds.{kind_name}.root must contain \"<name>\", got {:?}",
                kind.root
            ));
        }
        if !spec.skeletons.contains_key(&kind.skeleton) {
            problems.push(format!(
                "unit_kinds.{kind_name}.skeleton names unknown skeleton {:?}",
                kind.skeleton
            ));
        }
        match kind.members_source() {
            Err(e) => problems.push(format!("unit_kinds.{kind_name}.members: {e}")),
            Ok(super::spec::MembersSource::Registry)
            | Ok(super::spec::MembersSource::RegistryMetaDirs) => needs_registry = true,
            Ok(super::spec::MembersSource::Discover { .. }) => {}
            Ok(super::spec::MembersSource::Faces) => {
                // A faces-discovered kind whose skeleton names no faces has no
                // discriminator: it enrols nothing and every rule then reports
                // zero findings over zero units, which reads as conformance.
                if spec
                    .skeletons
                    .get(&kind.skeleton)
                    .is_some_and(|s| s.faces.is_empty())
                {
                    problems.push(format!(
                        "unit_kinds.{kind_name}.members is \"faces\" but skeleton {:?} \
                         declares none, so the kind would enrol nothing",
                        kind.skeleton
                    ));
                }
            }
        }
    }
    if needs_registry && spec.unit_registry.is_none() {
        problems.push(
            "a unit kind enumerates members from the registry but unit_registry is absent".into(),
        );
    }

    for (skel_name, skel) in &spec.skeletons {
        // Every skeleton, not only those a `faces` kind discovers on: face
        // dirs drive `unit_missing_face` and `face_edge_denied` whatever
        // enrolled the unit. See `unusable_face_dir`.
        let mut seen_dirs: BTreeMap<&str, &str> = BTreeMap::new();
        for (face, dir) in &skel.faces {
            if let Some(reason) = unusable_face_dir(dir) {
                problems.push(format!(
                    "skeletons.{skel_name}.faces.{face} is {dir:?}: {reason}"
                ));
            }
            if let Some(other) = seen_dirs.insert(dir.as_str(), face.as_str()) {
                problems.push(format!(
                    "skeletons.{skel_name}.faces.{face} and .{other} are both {dir:?}; \
                     two faces sharing one directory can never be told apart"
                ));
            }
        }
        let faces: BTreeSet<&str> = skel.faces.keys().map(String::as_str).collect();
        for f in &skel.required_faces {
            if !faces.contains(f.as_str()) {
                problems.push(format!(
                    "skeletons.{skel_name}.required_faces names unknown face {f:?}"
                ));
            }
        }
        for (from, tos) in &skel.face_dependency_matrix {
            if !faces.contains(from.as_str()) {
                problems.push(format!(
                    "skeletons.{skel_name}.face_dependency_matrix has unknown face {from:?}"
                ));
            }
            for to in tos {
                if !faces.contains(to.as_str()) {
                    problems.push(format!(
                        "skeletons.{skel_name}.face_dependency_matrix.{from} names unknown face {to:?}"
                    ));
                }
            }
        }
        let mut dirs: BTreeSet<String> = skel.faces.values().cloned().collect();
        for (class, sat) in &skel.satellites {
            if !dirs.insert(sat.dir.clone()) {
                problems.push(format!(
                    "skeletons.{skel_name}.satellites.{class}.dir {:?} collides with another face or satellite home; one canonical home per class",
                    sat.dir
                ));
            }
            if sat.form.trim().is_empty() {
                problems.push(format!(
                    "skeletons.{skel_name}.satellites.{class}.form must not be empty"
                ));
            }
            for alias in &sat.aliases {
                if alias == &sat.dir {
                    problems.push(format!(
                        "skeletons.{skel_name}.satellites.{class} lists its canonical dir as an alias"
                    ));
                }
            }
        }
    }

    for (suffix, face) in &spec.naming.face_by_suffix {
        if !spec.naming.layer_suffixes.iter().any(|s| s == suffix) {
            problems.push(format!(
                "naming.face_by_suffix has suffix {suffix:?} that is not in naming.layer_suffixes"
            ));
        }
        let known = spec.skeletons.values().any(|s| s.faces.contains_key(face));
        if !known {
            problems.push(format!(
                "naming.face_by_suffix.{suffix} names face {face:?} that no skeleton declares"
            ));
        }
    }

    if let Some(t) = &spec.naming.adapter_name
        && super::adapter_naming::template_separator(t).is_none()
    {
        problems.push(format!(
            "naming.adapter_name {t:?} must name two components separated by a literal, \
             as in \"<a>-<b>\"; the adapter naming rule has nothing to measure otherwise"
        ));
    }

    for (unit, over) in &spec.units {
        if unit.trim().is_empty() {
            problems.push("units has an empty unit name".into());
        }
        for class in &over.satellites_not_applicable {
            let declared = spec
                .skeletons
                .values()
                .any(|s| s.satellites.contains_key(class));
            if !declared {
                problems.push(format!(
                    "units.{unit}.satellites_not_applicable names undeclared satellite {class:?}"
                ));
            }
        }
    }

    for (rule, cfg) in &spec.rules {
        if cfg.frozen_empty && cfg.mode == RuleMode::AdvisoryUntilInfra {
            problems.push(format!(
                "rules.{rule}: frozen_empty has no meaning for an advisory rule"
            ));
        }
    }

    for r in &spec.root_files.rules {
        if r.value.trim().is_empty() {
            problems.push(format!("root_files rule {:?} has an empty value", r.id));
        }
    }

    problems
}

/// Why a face directory cannot match, or `None` if it can.
///
/// Written against oyatie's own `invalid_git_path` rather than invented: empty,
/// absolute, backslashes, and any `.`/`..`/empty segment. The trailing slash is
/// this engine's own requirement -- `TreeSource::has_dir` takes a prefix ending
/// in `/`, and without it the test degrades to a bare prefix match, so `core`
/// also matches a file `core.rs` and invents a unit.
///
/// Each rejected form was executed against `discover_units` and produced either
/// a phantom unit or none at all, with `validate` returning clean.
fn unusable_face_dir(dir: &str) -> Option<&'static str> {
    if dir.is_empty() {
        return Some("a face directory may not be empty");
    }
    if dir.starts_with('/') {
        return Some("a face directory must be relative to the unit root");
    }
    if dir.contains('\\') {
        return Some("a face directory must use '/' separators");
    }
    if !dir.ends_with('/') {
        return Some("a face directory must end with '/', or it matches files too");
    }
    // Trailing empty segment is the slash itself; every other one is a defect.
    let mut segments: Vec<&str> = dir.split('/').collect();
    segments.pop();
    if segments
        .iter()
        .any(|s| s.is_empty() || *s == "." || *s == "..")
    {
        return Some("a face directory may not contain an empty, '.' or '..' segment");
    }
    None
}
