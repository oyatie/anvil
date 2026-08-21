//! Cargo path dependencies as edges: `name = { path = ".." }` in
//! `[dependencies]` and `[build-dependencies]` (dev-dependencies are the
//! test harness, which is a primary adapter and may see anything), plus
//! `workspace = true` resolved through the root `[workspace.dependencies]`.

use super::normalize;
use crate::shape::ports::{
    DepEdge, DependencySource, LanguageProfile, ResolvedSpec, ResolvedUnit, SourceError, TreeSource,
};
use std::collections::BTreeMap;

pub struct CargoManifestDeps;

const SECTIONS: [&str; 2] = ["dependencies", "build-dependencies"];

fn path_of(v: &toml::Value) -> Option<String> {
    v.as_table()?.get("path")?.as_str().map(str::to_string)
}

impl DependencySource for CargoManifestDeps {
    fn profile(&self) -> LanguageProfile {
        LanguageProfile::RustCargo
    }

    fn edges(
        &self,
        tree: &dyn TreeSource,
        _spec: &ResolvedSpec,
        _units: &[ResolvedUnit],
    ) -> Result<Vec<DepEdge>, SourceError> {
        let marker = LanguageProfile::RustCargo.unit_marker();
        let manifests: Vec<(&String, &Vec<u8>)> = tree
            .loaded()
            .iter()
            .filter(|(p, _)| p.rsplit('/').next() == Some(marker))
            .collect();
        if manifests.is_empty() {
            return Err(SourceError::Unavailable(format!("no {marker} loaded")));
        }

        // Root workspace table for `workspace = true` resolution.
        let mut ws_paths: BTreeMap<String, String> = BTreeMap::new();
        if let Some(root) = tree.loaded().get(marker)
            && let Ok(text) = std::str::from_utf8(root)
            && let Ok(doc) = toml::from_str::<toml::Value>(text)
            && let Some(deps) = doc
                .get("workspace")
                .and_then(|w| w.get("dependencies"))
                .and_then(|d| d.as_table())
        {
            for (name, v) in deps {
                if let Some(p) = path_of(v) {
                    ws_paths.insert(name.clone(), normalize("", &p));
                }
            }
        }

        let mut edges = Vec::new();
        for (path, bytes) in manifests {
            let text = std::str::from_utf8(bytes)
                .map_err(|e| SourceError::Unavailable(format!("{path} is not UTF-8: {e}")))?;
            let doc: toml::Value = toml::from_str(text)
                .map_err(|e| SourceError::Unavailable(format!("{path} does not parse: {e}")))?;
            let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            let from = if dir.is_empty() {
                String::new()
            } else {
                format!("{dir}/")
            };
            for section in SECTIONS {
                let Some(table) = doc.get(section).and_then(|t| t.as_table()) else {
                    continue;
                };
                for (name, v) in table {
                    let to = if let Some(p) = path_of(v) {
                        Some(normalize(dir, &p))
                    } else if v
                        .as_table()
                        .and_then(|t| t.get("workspace"))
                        .and_then(|w| w.as_bool())
                        == Some(true)
                    {
                        ws_paths.get(name).cloned()
                    } else {
                        None
                    };
                    if let Some(to) = to {
                        edges.push(DepEdge {
                            from: from.clone(),
                            to,
                            via: format!("cargo path ({section})"),
                        });
                    }
                }
            }
        }
        Ok(edges)
    }
}
