use std::fs;
use std::path::{Path, PathBuf};

use super::workspace::PackageManifest;
use crate::source_scan::paths::module_graph::existing_file;

mod conventional;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TargetKind {
    Library,
    Binary,
    BuildScript,
}

pub(super) struct PackageRoot {
    pub(super) path: PathBuf,
    pub(super) kind: TargetKind,
}

pub(super) fn roots(
    manifest: &PackageManifest,
    repo_root: &Path,
    roots: &mut Vec<PackageRoot>,
) -> Result<(), String> {
    let package = manifest
        .value
        .get("package")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{} has no [package] table", manifest.manifest.display()))?;
    let package_name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| {
            format!(
                "package name in {} is missing or not a string",
                manifest.manifest.display()
            )
        })?;
    let library = manifest.value.get("lib").and_then(toml::Value::as_table);
    if let Some(path) = library.and_then(|table| table.get("path")) {
        let relative = path.as_str().ok_or_else(|| {
            format!(
                "manifest library path in {} is not a string",
                manifest.manifest.display()
            )
        })?;
        roots.push(PackageRoot {
            path: manifest_target(&manifest.directory, repo_root, relative, "library")?,
            kind: TargetKind::Library,
        });
    } else if library.is_some()
        || package
            .get("autolib")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true)
    {
        push_target_if_file(
            roots,
            manifest.directory.join("src/lib.rs"),
            repo_root,
            TargetKind::Library,
        )?;
    }

    if package
        .get("autobins")
        .and_then(toml::Value::as_bool)
        .unwrap_or(true)
    {
        push_target_if_file(
            roots,
            manifest.directory.join("src/main.rs"),
            repo_root,
            TargetKind::Binary,
        )?;
        push_conventional_targets(&manifest.directory, repo_root, roots)?;
    }
    let binaries = match manifest.value.get("bin") {
        Some(value) => Some(value.as_array().ok_or_else(|| {
            format!(
                "manifest `bin` in {} is not an array",
                manifest.manifest.display()
            )
        })?),
        None => None,
    };
    for target in binaries.into_iter().flatten() {
        let target = target.as_table().ok_or_else(|| {
            format!(
                "manifest binary target in {} is not a table",
                manifest.manifest.display()
            )
        })?;
        if let Some(path) = target.get("path") {
            let relative = path.as_str().ok_or_else(|| {
                format!(
                    "manifest binary path in {} is not a string",
                    manifest.manifest.display()
                )
            })?;
            roots.push(PackageRoot {
                path: manifest_target(&manifest.directory, repo_root, relative, "binary")?,
                kind: TargetKind::Binary,
            });
        } else {
            let name = target
                .get("name")
                .and_then(toml::Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "manifest binary target in {} has neither path nor name",
                        manifest.manifest.display()
                    )
                })?;
            roots.push(PackageRoot {
                path: conventional_named_bin(&manifest.directory, repo_root, package_name, name)?,
                kind: TargetKind::Binary,
            });
        }
    }

    match package.get("build") {
        Some(toml::Value::String(relative)) => roots.push(PackageRoot {
            path: manifest_target(&manifest.directory, repo_root, relative, "build-script")?,
            kind: TargetKind::BuildScript,
        }),
        Some(toml::Value::Boolean(true)) => roots.push(PackageRoot {
            path: manifest_target(&manifest.directory, repo_root, "build.rs", "build-script")?,
            kind: TargetKind::BuildScript,
        }),
        Some(toml::Value::Boolean(false)) => {}
        Some(value) => {
            return Err(format!(
                "package build target in {} must be a path string or false, got {value}",
                manifest.manifest.display()
            ));
        }
        None => push_target_if_file(
            roots,
            manifest.directory.join("build.rs"),
            repo_root,
            TargetKind::BuildScript,
        )?,
    }
    Ok(())
}

fn push_target_if_file(
    roots: &mut Vec<PackageRoot>,
    path: PathBuf,
    repo_root: &Path,
    kind: TargetKind,
) -> Result<(), String> {
    if existing_file(&path)? {
        roots.push(PackageRoot {
            path: contained_file(&path, repo_root)?,
            kind,
        });
    }
    Ok(())
}

fn push_conventional_targets(
    package_dir: &Path,
    repo_root: &Path,
    roots: &mut Vec<PackageRoot>,
) -> Result<(), String> {
    let paths = conventional::binary_roots(package_dir, repo_root)?;
    roots.extend(paths.into_iter().map(|path| PackageRoot {
        path,
        kind: TargetKind::Binary,
    }));
    Ok(())
}

pub(super) fn conventional_roots(
    package_dir: &Path,
    repo_root: &Path,
    roots: &mut Vec<PathBuf>,
) -> Result<(), String> {
    roots.extend(conventional::all_roots(package_dir, repo_root)?);
    Ok(())
}

fn manifest_target(
    package_dir: &Path,
    repo_root: &Path,
    relative: &str,
    kind: &str,
) -> Result<PathBuf, String> {
    let relative = Path::new(relative);
    if relative.is_absolute() {
        return Err(format!(
            "manifest {kind} path `{}` is absolute",
            relative.display()
        ));
    }
    let path = package_dir.join(relative);
    if !existing_file(&path)? {
        return Err(format!(
            "manifest {kind} source is missing at {}",
            path.display()
        ));
    }
    contained_file(&path, repo_root)
}

fn contained_file(path: &Path, repo_root: &Path) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("cannot resolve source {}: {error}", path.display()))?;
    if !canonical.starts_with(repo_root) {
        return Err(format!(
            "source {} escapes repository {}",
            path.display(),
            repo_root.display()
        ));
    }
    if !canonical.is_file() {
        return Err(format!("source {} is not a file", path.display()));
    }
    Ok(canonical)
}

fn conventional_named_bin(
    package_dir: &Path,
    repo_root: &Path,
    package_name: &str,
    name: &str,
) -> Result<PathBuf, String> {
    if name.is_empty() || name.contains(['/', '\\']) {
        return Err(format!(
            "manifest binary name `{name}` is not a single path component"
        ));
    }
    let bin_dir = package_dir.join("src/bin");
    let mut candidates = vec![
        bin_dir.join(format!("{name}.rs")),
        bin_dir.join(name).join("main.rs"),
    ];
    if name == package_name {
        candidates.push(package_dir.join("src/main.rs"));
    }
    let mut matches = Vec::new();
    for candidate in candidates {
        if existing_file(&candidate)? {
            matches.push(contained_file(&candidate, repo_root)?);
        }
    }
    if matches.len() != 1 {
        return Err(format!(
            "manifest binary `{name}` has {} conventional roots under {}",
            matches.len(),
            bin_dir.display()
        ));
    }
    Ok(matches.remove(0))
}

/// Root selected by Cargo's `src/bin/name.rs` or directory convention.
pub(in crate::source_scan::paths::module_graph) fn binary_subject_root(
    repo_root: &Path,
    segments: &[&str],
) -> Result<(Vec<PathBuf>, usize), String> {
    let Some(name) = segments.get(2) else {
        return Err("`src/bin` names no Cargo binary target".to_owned());
    };
    let root = fs::canonicalize(repo_root)
        .map_err(|error| format!("cannot resolve repository {}: {error}", repo_root.display()))?;
    let bin = root.join("src/bin");
    let mut roots = Vec::new();
    for candidate in [
        bin.join(format!("{name}.rs")),
        bin.join(name).join("main.rs"),
    ] {
        conventional::push_if_file(&mut roots, candidate, &root)?;
    }
    if roots.len() != 1 {
        return Err(format!(
            "Cargo binary `{name}` has {} roots under {}",
            roots.len(),
            bin.display()
        ));
    }
    let child_start = if segments.get(3) == Some(&"main") {
        4
    } else {
        3
    };
    Ok((roots, child_start))
}
