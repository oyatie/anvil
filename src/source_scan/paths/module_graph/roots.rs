use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use syn::Item;

use super::read_parsed;
use crate::source_scan::cfg::excludes_when_test_is_false;

mod package;
mod workspace;

pub(super) use package::binary_subject_root;

pub(super) struct CrateRoot {
    pub(super) path: PathBuf,
    pub(super) aliases: BTreeSet<String>,
    pub(super) audited_derive_crates: BTreeMap<String, String>,
}

/// Architecture-only descriptors keep the existing declaration context API
/// unchanged while retaining concrete target identity for local aliases.
pub(super) fn architecture_roots(
    repo: &Path,
) -> Result<Vec<super::ownership::OwnershipRoot>, String> {
    use super::ownership::{CrateIdentity, OwnershipRoot};
    let Some(packages) = workspace::discover(repo)? else {
        let mut paths = Vec::new();
        package::conventional_roots(repo, repo, &mut paths)?;
        return Ok(paths
            .into_iter()
            .map(|root| OwnershipRoot {
                identity: CrateIdentity {
                    manifest: None,
                    root,
                    kind: "conventional",
                },
                aliases: BTreeMap::new(),
            })
            .collect());
    };
    let mut targets = Vec::new();
    for (index, manifest) in packages.iter().enumerate() {
        let mut roots = Vec::new();
        package::roots(manifest, repo, &mut roots)?;
        for target in roots {
            let kind = match target.kind {
                package::TargetKind::Library => "library",
                package::TargetKind::Binary => "binary",
                package::TargetKind::BuildScript => "build-script",
            };
            let identity = CrateIdentity {
                manifest: Some(manifest.manifest.clone()),
                root: target.path.clone(),
                kind,
            };
            targets.push((index, target, identity));
        }
    }
    let libraries: BTreeMap<_, _> = targets
        .iter()
        .filter(|(_, target, _)| target.kind == package::TargetKind::Library)
        .map(|(index, _, identity)| (packages[*index].directory.clone(), identity.clone()))
        .collect();
    let mut out = Vec::new();
    for (index, target, identity) in targets {
        let names = packages[index].local_alias_targets(target.kind, &packages)?;
        let mut aliases = BTreeMap::new();
        for (name, destinations) in names {
            let resolved = destinations
                .iter()
                .map(|directory| {
                    libraries
                        .get(directory)
                        .cloned()
                        .ok_or_else(|| "local alias destination has no library target".to_string())
                })
                .collect::<Result<BTreeSet<_>, _>>()?;
            aliases.insert(name, resolved);
        }
        out.push(OwnershipRoot { identity, aliases });
    }
    Ok(out)
}

/// Top-level production modules declared by every package in the workspace.
pub fn production_top_level_modules(repo_root: &Path) -> Result<BTreeSet<String>, String> {
    let mut modules = BTreeSet::new();
    for root in all_crate_roots(repo_root)? {
        let (_, parsed) = read_parsed(&root)?;
        for item in parsed.items {
            let Item::Mod(module) = item else { continue };
            if !excludes_when_test_is_false(&module.attrs) {
                modules.insert(module.ident.to_string());
            }
        }
    }
    if modules.is_empty() {
        return Err(format!(
            "no production Rust modules are declared under {}",
            repo_root.display()
        ));
    }
    Ok(modules)
}

pub(super) fn crate_roots(repo_root: &Path) -> Result<Vec<PathBuf>, String> {
    let roots = all_crate_roots(repo_root)?;
    if roots.is_empty() {
        return Err(format!(
            "no readable Rust crate root under {}",
            repo_root.display()
        ));
    }
    Ok(roots)
}

/// Every library, binary, or build-script root Cargo builds for packages in this workspace.
pub(super) fn all_crate_roots(repo_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut roots = crate_roots_with_context(repo_root)?
        .into_iter()
        .map(|root| root.path)
        .collect::<Vec<_>>();
    roots.sort();
    roots.dedup();
    Ok(roots)
}

pub(super) fn crate_roots_with_context(repo_root: &Path) -> Result<Vec<CrateRoot>, String> {
    let canonical_repo = fs::canonicalize(repo_root)
        .map_err(|error| format!("cannot resolve repository {}: {error}", repo_root.display()))?;
    let mut contexts = Vec::new();
    match workspace::discover(&canonical_repo)? {
        Some(packages) => {
            for manifest in &packages {
                let mut roots = Vec::new();
                package::roots(manifest, &canonical_repo, &mut roots)?;
                for root in roots {
                    contexts.push(CrateRoot {
                        path: root.path,
                        aliases: manifest.crate_aliases(root.kind, &packages)?,
                        audited_derive_crates: manifest.audited_derive_crates(root.kind),
                    });
                }
            }
        }
        None => {
            let mut roots = Vec::new();
            package::conventional_roots(&canonical_repo, &canonical_repo, &mut roots)?;
            contexts.extend(roots.into_iter().map(|path| CrateRoot {
                path,
                aliases: BTreeSet::new(),
                audited_derive_crates: BTreeMap::new(),
            }));
        }
    }
    contexts.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.aliases.cmp(&right.aliases))
            .then_with(|| left.audited_derive_crates.cmp(&right.audited_derive_crates))
    });
    contexts.dedup_by(|left, right| {
        left.path == right.path
            && left.aliases == right.aliases
            && left.audited_derive_crates == right.audited_derive_crates
    });
    Ok(contexts)
}
