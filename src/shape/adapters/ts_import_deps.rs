//! TypeScript and JavaScript module specifiers as dependency edges, for the
//! `ts-workspace` profile.
//!
//! Two kinds of specifier carry a build edge, and both are read. A relative
//! specifier resolves against the importing file's own directory, which is
//! how a unit's faces reach each other. A bare specifier resolves through the
//! `name` a workspace manifest declares, which is how one unit reaches
//! another — reading only the relative half would miss exactly the edges the
//! cross-unit rule exists for.
//!
//! # What is deliberately not an edge
//!
//! A bare specifier no manifest in the tree claims is an external package, so
//! it is skipped rather than pointed at a directory that does not exist. What
//! this cannot do is resolve a `paths` alias from a TypeScript compiler
//! configuration; an alias reads as a bare specifier and, unless a manifest
//! claims that name, is skipped as external.
//!
//! # Absence is not conformance (I1)
//!
//! A tree with no workspace manifest, or with no source file under the unit
//! roots, is `Unavailable`: the dependency rules are then reported as not
//! measured rather than as a graph with no violations.

use super::normalize;
use crate::shape::ports::{
    DepEdge, DependencySource, LanguageProfile, ResolvedSpec, ResolvedUnit, SourceError, TreeSource,
};
use std::collections::BTreeMap;

pub struct TsImportDeps;

const SOURCE_EXTENSIONS: [&str; 6] = [".ts", ".tsx", ".mts", ".js", ".jsx", ".mjs"];

/// Whether a path is a source file this adapter reads.
pub fn is_source(path: &str) -> bool {
    !path.ends_with(".d.ts") && SOURCE_EXTENSIONS.iter().any(|e| path.ends_with(e))
}

/// Package name -> its directory, with a trailing slash, from every workspace
/// manifest loaded.
fn packages(tree: &dyn TreeSource) -> BTreeMap<String, String> {
    let marker = LanguageProfile::TsWorkspace.unit_marker();
    let mut out = BTreeMap::new();
    for (path, bytes) in tree.loaded() {
        if path.rsplit('/').next() != Some(marker) {
            continue;
        }
        let Ok(doc) = serde_json::from_slice::<serde_json::Value>(bytes) else {
            continue;
        };
        let Some(name) = doc.get("name").and_then(|n| n.as_str()) else {
            continue;
        };
        // The manifest at the repository root names the workspace itself,
        // not a unit; an edge to "" would point at every unit at once.
        let Some((dir, _)) = path.rsplit_once('/') else {
            continue;
        };
        out.insert(name.to_string(), normalize("", dir));
    }
    out
}

/// The quoted module specifiers a line imports.
///
/// Scoped to lines that open an `import`/`export` statement or call `require`,
/// so a quoted string elsewhere in the file is not mistaken for a specifier.
fn specifiers(line: &str) -> Vec<&str> {
    let t = line.trim_start();
    let importing = t.starts_with("import")
        || t.starts_with("export")
        || t.contains("require(")
        || t.contains("import(");
    if !importing {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut rest = t;
    while let Some(open) = rest.find(['"', '\'']) {
        let quote = &rest[open..open + 1];
        let after = &rest[open + 1..];
        let Some(close) = after.find(quote) else {
            break;
        };
        out.push(&after[..close]);
        rest = &after[close + 1..];
    }
    out
}

/// The directory a specifier names, relative to the importing file's directory.
///
/// A final segment carrying a dot is a file name and is dropped: the face a
/// path belongs to is decided by its directory, and an edge naming a file
/// would key the finding on something the tenant may rename freely.
fn target_dir(dir: &str, spec: &str, packages: &BTreeMap<String, String>) -> Option<String> {
    if !spec.starts_with('.') {
        return packages
            .iter()
            .filter(|(name, _)| spec == name.as_str() || spec.starts_with(&format!("{name}/")))
            .max_by_key(|(name, _)| name.len())
            .map(|(_, d)| d.clone());
    }
    let trimmed = match spec.rsplit_once('/') {
        Some((head, last)) if last.contains('.') && !last.starts_with('.') => head,
        _ => spec,
    };
    Some(normalize(dir, trimmed))
}

impl DependencySource for TsImportDeps {
    fn profile(&self) -> LanguageProfile {
        LanguageProfile::TsWorkspace
    }

    fn edges(
        &self,
        tree: &dyn TreeSource,
        _spec: &ResolvedSpec,
        _units: &[ResolvedUnit],
    ) -> Result<Vec<DepEdge>, SourceError> {
        let marker = LanguageProfile::TsWorkspace.unit_marker();
        let packages = packages(tree);
        if packages.is_empty() {
            return Err(SourceError::Unavailable(format!(
                "no {marker} declaring a name was loaded, so a bare import cannot be resolved"
            )));
        }
        let mut edges = Vec::new();
        let mut any = false;
        for (path, bytes) in tree.loaded() {
            if !is_source(path) {
                continue;
            }
            any = true;
            let Ok(text) = std::str::from_utf8(bytes) else {
                continue;
            };
            let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
            for line in text.lines() {
                for spec in specifiers(line) {
                    let Some(to) = target_dir(dir, spec, &packages) else {
                        continue;
                    };
                    edges.push(DepEdge {
                        from: path.clone(),
                        to,
                        via: "ts import".to_string(),
                    });
                }
            }
        }
        if !any {
            return Err(SourceError::Unavailable(
                "no TypeScript or JavaScript source file was loaded".into(),
            ));
        }
        Ok(edges)
    }
}
