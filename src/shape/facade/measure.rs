//! Measure one repository at one revision: locate the spec (in the tree, or
//! supplied as a proposal), load only the files the spec implies, resolve the
//! unit registry, read dependency edges, and run the engine.

use super::cli::SPEC_PATH;
use crate::shape::adapters::{
    BuckLabelDeps, CargoManifestDeps, GitTreeAtRev, RustUseDeps, TsImportDeps,
};
use crate::shape::ports::{
    DepGraph, DependencySource, LanguageProfile, ResolvedSpec, ShapeReport, ShapeSpec, SpecSource,
    TreeSource, discover_units, measure, resolve,
};
use anyhow::{Context, Result, anyhow};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub struct MeasureRequest {
    pub repo_dir: PathBuf,
    pub rev: String,
    /// Label for the report (e.g. `oyatie/oyatie`).
    pub repo: String,
    /// A spec supplied from outside the tree; the report is stamped Proposed.
    pub spec_override: Option<PathBuf>,
    /// A registry document supplied from outside the tree.
    pub registry_override: Option<PathBuf>,
}

/// Which tree paths the engine needs loaded for `spec`: its own config, the
/// registry, every profile's manifests and markers, and the source files the
/// dependency and port rules read (every `.rs` under an adapters face; every
/// `.rs` under a module-tree root).
pub fn selector(spec: &ShapeSpec) -> impl Fn(&str) -> bool {
    let mut basenames: BTreeSet<String> = spec
        .profiles
        .iter()
        .map(|p| p.unit_marker().to_string())
        .collect();
    basenames.insert(LanguageProfile::RustCargo.unit_marker().to_string());
    for skel in spec.skeletons.values() {
        if let Some(m) = &skel.unit_marker {
            basenames.insert(m.clone());
        }
    }
    for kind in spec.unit_kinds.values() {
        if let Some(m) = kind.members.strip_prefix("discover:") {
            basenames.insert(m.trim().to_string());
        }
    }
    let registry_path = spec.unit_registry.as_ref().map(|r| r.path.clone());
    let adapters_dirs: Vec<String> = spec
        .skeletons
        .values()
        .filter_map(|s| s.faces.get("adapters"))
        .map(|d| format!("/{}/", d.trim_end_matches('/')))
        .collect();
    let module_roots: Vec<String> = if spec.profiles.contains(&LanguageProfile::RustModuleTree) {
        spec.unit_kinds
            .values()
            .filter_map(|k| k.root.split_once("<name>").map(|(p, _)| p.to_string()))
            .collect()
    } else {
        Vec::new()
    };
    move |p: &str| {
        p == SPEC_PATH
            || registry_path.as_deref() == Some(p)
            || p.rsplit('/').next().is_some_and(|b| basenames.contains(b))
            || (p.ends_with(".rs")
                && (adapters_dirs.iter().any(|d| p.contains(d.as_str()))
                    || module_roots.iter().any(|r| p.starts_with(r.as_str()))))
    }
}

pub async fn measure_repo(req: &MeasureRequest) -> Result<ShapeReport> {
    let (spec, source) = load_spec(req).await?;
    let tree = GitTreeAtRev::load(&req.repo_dir, &req.rev, selector(&spec))
        .await
        .map_err(|e| anyhow!("{e}"))?;

    let registry_path = spec.unit_registry.as_ref().map(|r| r.path.clone());
    let registry = match (&req.registry_override, &registry_path) {
        (Some(p), _) => Some(read_json(p)?),
        (None, Some(path)) => match tree.read(path) {
            Ok(Some(bytes)) => Some(
                serde_json::from_slice(bytes)
                    .with_context(|| format!("registry {path} at {}", tree.rev()))?,
            ),
            _ => None,
        },
        (None, None) => None,
    };
    let resolved = resolve(&spec, registry.as_ref()).map_err(|e| anyhow!("{e}"))?;
    let deps = dependency_graph(&resolved, &tree);
    Ok(measure(&resolved, &tree, &req.repo, source, &deps))
}

/// Edges from every adapter the spec's profiles name; a profile whose
/// adapter cannot read the tree is recorded as unavailable.
pub fn dependency_graph(resolved: &ResolvedSpec, tree: &dyn TreeSource) -> DepGraph {
    let units = discover_units(resolved, tree);
    let mut graph = DepGraph::default();
    for profile in &resolved.spec.profiles {
        let source: Box<dyn DependencySource> = match profile {
            LanguageProfile::RustCargo => Box::new(CargoManifestDeps),
            LanguageProfile::RustBuck2 => Box::new(BuckLabelDeps),
            LanguageProfile::RustModuleTree => Box::new(RustUseDeps),
            LanguageProfile::TsWorkspace => Box::new(TsImportDeps),
        };
        match source.edges(tree, resolved, &units) {
            Ok(edges) => graph.edges.extend(edges),
            Err(e) => graph.unavailable.push((*profile, e.to_string())),
        }
    }
    graph.edges.sort();
    graph.edges.dedup();
    graph
}

async fn load_spec(req: &MeasureRequest) -> Result<(ShapeSpec, SpecSource)> {
    if let Some(p) = &req.spec_override {
        let raw = std::fs::read_to_string(p).with_context(|| p.display().to_string())?;
        let spec = ShapeSpec::parse(&raw).map_err(|e| anyhow!("{e}"))?;
        return Ok((spec, SpecSource::Proposed(p.display().to_string())));
    }
    let probe = GitTreeAtRev::load(&req.repo_dir, &req.rev, |p| p == SPEC_PATH)
        .await
        .map_err(|e| anyhow!("{e}"))?;
    match probe.read(SPEC_PATH) {
        Ok(Some(bytes)) => {
            let raw = std::str::from_utf8(bytes).context("spec is not UTF-8")?;
            let spec = ShapeSpec::parse(raw).map_err(|e| anyhow!("{e}"))?;
            Ok((spec, SpecSource::Adopted))
        }
        _ => Err(anyhow!(
            "no {SPEC_PATH} at {} in {}; pass --spec-override to measure against a proposal",
            probe.rev(),
            req.repo_dir.display()
        )),
    }
}

fn read_json(p: &Path) -> Result<serde_json::Value> {
    let raw = std::fs::read_to_string(p).with_context(|| p.display().to_string())?;
    serde_json::from_str(&raw).with_context(|| p.display().to_string())
}

/// Human summary for the CLI.
pub fn render(report: &ShapeReport) -> String {
    use std::collections::BTreeMap;
    let d = report.distance();
    let mut per_rule: BTreeMap<&str, usize> = BTreeMap::new();
    for f in &report.findings {
        *per_rule.entry(f.rule.0.as_str()).or_default() += 1;
    }
    let mut out = format!(
        "shape: {} @ {} ({})\n  units: {} ({} conformant)\n  findings: {} ({} misplaced files, {} denied edges)\n",
        report.repo,
        &report.rev[..report.rev.len().min(12)],
        match &report.spec_source {
            SpecSource::Adopted => "adopted spec".to_string(),
            SpecSource::Proposed(p) => format!("PROPOSED spec {p}"),
            SpecSource::CandidateBootstrap => "candidate spec (none at merge-base)".to_string(),
        },
        d.units_total,
        d.units_conformant,
        d.findings_total,
        d.files_misplaced,
        d.edges_denied
    );
    for (rule, n) in &per_rule {
        out.push_str(&format!("    {rule:<32} {n}\n"));
    }
    if !report.not_measured.is_empty() {
        out.push_str("  not measured:\n");
        for (rule, why) in &report.not_measured {
            out.push_str(&format!("    {:<32} {why}\n", rule.0));
        }
    }
    out
}
