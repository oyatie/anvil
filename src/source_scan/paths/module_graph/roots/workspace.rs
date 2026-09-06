use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use super::package::TargetKind;

mod aliases;
mod audited_registry;
mod authority;
mod dependencies;
mod glob;
mod provenance;
pub(super) use aliases::normalize_crate_name;
use dependencies::{DependencySpec, LocalOverride};
use glob::{excluded, expand_member_pattern};

pub(super) struct PackageManifest {
    pub(super) directory: PathBuf,
    pub(super) manifest: PathBuf,
    pub(super) value: toml::Value,
    dependencies: Vec<DependencySpec>,
    global_overrides: Vec<LocalOverride>,
    audited_registry: BTreeSet<(String, dependencies::DependencyKind, Option<String>)>,
}

pub(super) fn discover(repo_root: &Path) -> Result<Option<Vec<PackageManifest>>, String> {
    let root_manifest = repo_root.join("Cargo.toml");
    if !root_manifest.is_file() {
        return Ok(None);
    }
    let canonical_repo = fs::canonicalize(repo_root)
        .map_err(|error| format!("cannot resolve repository {}: {error}", repo_root.display()))?;
    contained_manifest(&canonical_repo, &root_manifest)?;
    let root_value = read_manifest(&root_manifest)?;
    let workspace = match root_value.get("workspace") {
        Some(value) => Some(
            value
                .as_table()
                .ok_or_else(|| "root manifest `workspace` is not a table".to_owned())?,
        ),
        None => None,
    };
    if workspace.is_none() && root_value.get("package").is_none() {
        return Err(format!(
            "root manifest {} has neither [package] nor [workspace]",
            root_manifest.display()
        ));
    }
    let excludes = string_array(workspace.and_then(|table| table.get("exclude")), "exclude")?;
    let members = string_array(workspace.and_then(|table| table.get("members")), "members")?;

    let mut queue = VecDeque::new();
    let mut member_dirs = BTreeSet::new();
    if root_value.get("package").is_some() {
        queue.push_back(canonical_repo.clone());
        member_dirs.insert(canonical_repo.clone());
    }
    for pattern in members {
        let matches = expand_member_pattern(&canonical_repo, &pattern)?;
        if matches.is_empty() {
            return Err(format!(
                "workspace member pattern `{pattern}` matched no paths"
            ));
        }
        let literal = !pattern.contains(['*', '?', '[']);
        for directory in matches {
            let relative = relative_string(&canonical_repo, &directory)?;
            if !literal && excluded(&relative, &excludes)? {
                continue;
            }
            let directory = canonical_package_dir(&canonical_repo, &directory)?;
            member_dirs.insert(directory.clone());
            queue.push_back(directory);
        }
    }

    let global_overrides = dependencies::overrides(&root_value, &canonical_repo)?;
    for dependency in &global_overrides {
        queue.push_back(canonical_package_dir(
            &canonical_repo,
            &dependency.directory,
        )?);
    }

    let workspace_dependencies = workspace
        .and_then(|table| table.get("dependencies"))
        .and_then(toml::Value::as_table);
    let mut seen = BTreeSet::new();
    let mut packages = Vec::new();
    while let Some(directory) = queue.pop_front() {
        if !seen.insert(directory.clone()) {
            continue;
        }
        let manifest = directory.join("Cargo.toml");
        let value = if directory == canonical_repo {
            root_value.clone()
        } else {
            read_manifest(&manifest)?
        };
        if value
            .get("package")
            .and_then(toml::Value::as_table)
            .is_none()
        {
            return Err(format!(
                "workspace member manifest {} has no [package] table",
                manifest.display()
            ));
        }
        let inherited = if member_dirs.contains(&directory) {
            workspace_dependencies
        } else {
            None
        };
        let dependencies = dependencies::collect(&value, inherited, &canonical_repo, &directory)?;
        for dependency in dependencies
            .iter()
            .filter_map(|dependency| dependency.directory.as_ref())
        {
            let dependency = canonical_package_dir(&canonical_repo, dependency)?;
            let relative = relative_string(&canonical_repo, &dependency)?;
            if !excluded(&relative, &excludes)? {
                member_dirs.insert(dependency.clone());
            }
            // Workspace exclusion controls membership/inheritance, not
            // whether an explicit path dependency is compiled.
            queue.push_back(dependency);
        }
        packages.push(PackageManifest {
            directory,
            manifest,
            value,
            dependencies,
            global_overrides: global_overrides.clone(),
            audited_registry: BTreeSet::new(),
        });
    }
    packages.sort_by(|a, b| a.manifest.cmp(&b.manifest));
    authority::admit(&canonical_repo, &mut packages);
    Ok(Some(packages))
}

fn read_manifest(path: &Path) -> Result<toml::Value, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    let text = String::from_utf8(bytes)
        .map_err(|error| format!("{} is not UTF-8: {error}", path.display()))?;
    text.parse::<toml::Value>()
        .map_err(|error| format!("cannot parse {}: {error}", path.display()))
}

fn string_array(value: Option<&toml::Value>, name: &str) -> Result<Vec<String>, String> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| format!("workspace `{name}` must be an array"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("workspace `{name}` entries must be strings"))
        })
        .collect()
}

fn canonical_package_dir(root: &Path, directory: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(directory).map_err(|error| {
        format!(
            "cannot resolve package directory {}: {error}",
            directory.display()
        )
    })?;
    if !canonical.starts_with(root) {
        return Err(format!(
            "package directory {} escapes repository {}",
            directory.display(),
            root.display()
        ));
    }
    if !canonical.is_dir() || !canonical.join("Cargo.toml").is_file() {
        return Err(format!(
            "workspace package {} has no Cargo.toml",
            directory.display()
        ));
    }
    contained_manifest(root, &canonical.join("Cargo.toml"))?;
    Ok(canonical)
}

fn contained_manifest(root: &Path, manifest: &Path) -> Result<(), String> {
    let canonical = fs::canonicalize(manifest)
        .map_err(|error| format!("cannot resolve manifest {}: {error}", manifest.display()))?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(format!(
            "manifest {} escapes repository {}",
            manifest.display(),
            root.display()
        ));
    }
    Ok(())
}

fn relative_string(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map_err(|_| format!("{} is outside {}", path.display(), root.display()))?
        .to_str()
        .map(|path| path.replace('\\', "/"))
        .ok_or_else(|| format!("package path {} is not UTF-8", path.display()))
}
