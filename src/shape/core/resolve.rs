//! Resolution materialises the unit list a spec refers to. Units that come
//! from a registry are listed here; units that are discovered by a marker
//! file become `DiscoveryRule`s the measurement engine applies to the tree.
//! Core never reads files: the registry document is passed in.

use super::spec::{MembersSource, ShapeSpec, SpecError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedUnit {
    pub name: String,
    pub kind: String,
    /// Unit root relative to the repository root, with a trailing slash.
    pub root: String,
    pub skeleton: String,
    pub destination_stable: bool,
    pub satellites_not_applicable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryRule {
    pub kind: String,
    pub root_pattern: String,
    pub marker: String,
    pub skeleton: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSpec {
    pub spec: ShapeSpec,
    pub units: Vec<ResolvedUnit>,
    pub discovery: Vec<DiscoveryRule>,
    /// Face names the registry declares, when it declares any; the engine
    /// reports a skeleton face the registry does not know as a spec problem.
    pub registry_faces: Option<Vec<String>>,
}

pub fn resolve(
    spec: &ShapeSpec,
    registry: Option<&serde_json::Value>,
) -> Result<ResolvedSpec, SpecError> {
    let mut units = Vec::new();
    let mut discovery = Vec::new();

    for (kind_name, kind) in &spec.unit_kinds {
        let source = kind
            .members_source()
            .map_err(|e| SpecError::Invalid(vec![e]))?;
        match source {
            MembersSource::Discover { marker } => discovery.push(DiscoveryRule {
                kind: kind_name.clone(),
                root_pattern: kind.root.clone(),
                marker,
                skeleton: kind.skeleton.clone(),
            }),
            MembersSource::Registry | MembersSource::RegistryMetaDirs => {
                let reg_ref = spec
                    .unit_registry
                    .as_ref()
                    .ok_or_else(|| SpecError::Registry("spec has no unit_registry".to_string()))?;
                let doc = registry.ok_or_else(|| {
                    SpecError::Registry(format!(
                        "unit kind {kind_name:?} enumerates members from {} but no registry document was supplied",
                        reg_ref.path
                    ))
                })?;
                let list = match source {
                    MembersSource::Registry => &reg_ref.units,
                    _ => reg_ref.meta_dirs.as_ref().ok_or_else(|| {
                        SpecError::Registry(format!(
                            "unit kind {kind_name:?} uses registry-meta-dirs but unit_registry.meta_dirs is absent"
                        ))
                    })?,
                };
                for name in names_at(doc, &list.pointer, &list.key)? {
                    let over = spec.units.get(&name);
                    units.push(ResolvedUnit {
                        root: kind.root.replace("<name>", &name),
                        name: name.clone(),
                        kind: kind_name.clone(),
                        skeleton: kind.skeleton.clone(),
                        destination_stable: over
                            .and_then(|o| o.destination_stable)
                            .unwrap_or(spec.destination_stable_default),
                        satellites_not_applicable: over
                            .map(|o| o.satellites_not_applicable.clone())
                            .unwrap_or_default(),
                    });
                }
            }
        }
    }

    let registry_faces = match (&spec.unit_registry, registry) {
        (Some(reg_ref), Some(doc)) => match &reg_ref.faces {
            Some(list) => Some(names_at(doc, &list.pointer, &list.key)?),
            None => None,
        },
        _ => None,
    };

    Ok(ResolvedSpec {
        spec: spec.clone(),
        units,
        discovery,
        registry_faces,
    })
}

/// The `key` field of every element of the list at `pointer`. Elements that
/// are bare strings are accepted as names.
fn names_at(doc: &serde_json::Value, pointer: &str, key: &str) -> Result<Vec<String>, SpecError> {
    let list = doc.pointer(pointer).ok_or_else(|| {
        SpecError::Registry(format!("registry has nothing at pointer {pointer:?}"))
    })?;
    let arr = list.as_array().ok_or_else(|| {
        SpecError::Registry(format!("registry value at {pointer:?} is not a list"))
    })?;
    let mut names = Vec::with_capacity(arr.len());
    for (i, el) in arr.iter().enumerate() {
        let name = match el {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(o) => o
                .get(key)
                .and_then(|v| v.as_str())
                .map(str::to_string)
                .ok_or_else(|| {
                    SpecError::Registry(format!(
                        "registry element {pointer}/{i} has no string field {key:?}"
                    ))
                })?,
            _ => {
                return Err(SpecError::Registry(format!(
                    "registry element {pointer}/{i} is neither a string nor an object"
                )))
            }
        };
        if name.trim().is_empty() {
            return Err(SpecError::Registry(format!(
                "registry element {pointer}/{i} has an empty {key:?}"
            )));
        }
        names.push(name);
    }
    Ok(names)
}
