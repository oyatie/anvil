//! `use crate::<unit>::<face>...` paths as edges, for a single crate whose
//! units are modules under a root (the `rust-module-tree` profile). The
//! target is `<root><unit>/<seg>/` when `<seg>` is a face directory of that
//! unit's skeleton, else `<root><unit>/`.

use crate::shape::ports::{
    DepEdge, DependencySource, LanguageProfile, ResolvedSpec, ResolvedUnit, SourceError, TreeSource,
};

pub struct RustUseDeps;

impl DependencySource for RustUseDeps {
    fn profile(&self) -> LanguageProfile {
        LanguageProfile::RustModuleTree
    }

    fn edges(
        &self,
        tree: &dyn TreeSource,
        spec: &ResolvedSpec,
        units: &[ResolvedUnit],
    ) -> Result<Vec<DepEdge>, SourceError> {
        // The module-tree root is the common prefix of the discovered roots
        // ("src/" for "src/<name>/").
        let Some(root) = spec
            .discovery
            .iter()
            .filter(|d| d.marker == LanguageProfile::RustModuleTree.unit_marker())
            .filter_map(|d| {
                d.root_pattern
                    .split_once("<name>")
                    .map(|(p, _)| p.to_string())
            })
            .next()
        else {
            return Err(SourceError::Unavailable(
                "no unit kind is discovered by the module-tree marker".into(),
            ));
        };
        let mut edges = Vec::new();
        let mut any = false;
        for (path, bytes) in tree.loaded() {
            if !path.ends_with(".rs") || !path.starts_with(root.as_str()) {
                continue;
            }
            any = true;
            let Ok(text) = std::str::from_utf8(bytes) else {
                continue;
            };
            for line in text.lines() {
                let t = line.trim_start();
                let t = t
                    .strip_prefix("pub(crate) ")
                    .or_else(|| t.strip_prefix("pub "))
                    .unwrap_or(t);
                let Some(rest) = t.strip_prefix("use crate::") else {
                    continue;
                };
                let mut segs = rest.split("::");
                let Some(unit_name) = segs.next() else {
                    continue;
                };
                let unit_name =
                    unit_name.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_');
                let Some(unit) = units
                    .iter()
                    .find(|u| u.root == format!("{root}{unit_name}/"))
                else {
                    continue;
                };
                let face_seg = segs.next().map(|s| {
                    s.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_')
                        .to_string()
                });
                let skel = spec.spec.skeletons.get(&unit.skeleton);
                let to = match (face_seg, skel) {
                    (Some(seg), Some(sk))
                        if sk.faces.values().any(|d| d.trim_end_matches('/') == seg) =>
                    {
                        format!("{}{seg}/", unit.root)
                    }
                    _ => unit.root.clone(),
                };
                edges.push(DepEdge {
                    from: path.clone(),
                    to,
                    via: "use crate::".to_string(),
                });
            }
        }
        if !any {
            return Err(SourceError::Unavailable(format!(
                "no .rs files loaded under {root}"
            )));
        }
        Ok(edges)
    }
}
