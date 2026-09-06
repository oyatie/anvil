// Fixed source-selection evidence, never permission to execute a contributor macro.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use super::dependencies::DependencyKind;
use super::{PackageManifest, audited_registry, normalize_crate_name};

pub(super) mod metadata;
mod snapshot;
#[cfg(test)]
use metadata::Node;
use metadata::{Metadata, selected_dependency};
use snapshot::Snapshot;
#[cfg(test)]
use snapshot::{configuration_absent, supported_manifest};

type Bindings = BTreeMap<PathBuf, BTreeSet<(String, DependencyKind, Option<String>)>>;
static CACHE: Mutex<Option<(Snapshot, Bindings)>> = Mutex::new(None);

pub(super) fn admit(root: &Path, packages: &mut [PackageManifest]) {
    // Ordinary dependency-free fixtures need no external macro authority.
    if !packages.iter().any(|package| {
        package
            .dependencies
            .iter()
            .any(|dep| dep.audited_macro_surface_enabled())
    }) {
        return;
    }
    let Some(before) = Snapshot::read(root, packages) else {
        return;
    };
    let Ok(mut cache) = CACHE.lock() else { return };
    let bindings = if let Some((snapshot, bindings)) =
        cache.as_ref().filter(|(snapshot, _)| snapshot == &before)
    {
        let _ = snapshot;
        bindings.clone()
    } else {
        let Some(bindings) = resolve(root, packages, &before) else {
            return;
        };
        // Bound reuse to one content snapshot. Registry source immutability and
        // a trusted toolchain/cache are approved assumptions, not host attestation.
        if Snapshot::read(root, packages).as_ref() != Some(&before) {
            return;
        }
        *cache = Some((before, bindings.clone()));
        bindings
    };
    for package in packages {
        package.audited_registry = bindings.get(&package.manifest).cloned().unwrap_or_default();
    }
}

fn metadata_command(root: &Path, cargo_home: &Path, rustup_home: &Path) -> std::process::Command {
    let mut command = crate::exec::build_env::command("cargo");
    command.current_dir(root).args([
        "metadata",
        "--locked",
        "--offline",
        "--format-version",
        "1",
        "--all-features",
    ]);
    // Own rustup's selection before Cargo sees --offline; contributor toolchain
    // files and ambient RUSTUP_TOOLCHAIN cannot trigger another toolchain/install.
    command
        .env("RUSTUP_TOOLCHAIN", "1.98.0")
        .env("RUSTUP_AUTO_INSTALL", "0")
        .env("CARGO_HOME", cargo_home)
        .env("RUSTUP_HOME", rustup_home);
    command.into_std()
}

fn resolve(root: &Path, packages: &[PackageManifest], snapshot: &Snapshot) -> Option<Bindings> {
    let output = crate::exec::run_sync_bounded(
        metadata_command(root, &snapshot.cargo_home, &snapshot.rustup_home),
        Duration::from_secs(30),
        "classification Cargo metadata",
    )
    .ok()?;
    if !output.status.success() {
        return None;
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout).ok()?;
    if std::fs::canonicalize(&metadata.workspace_root)
        .ok()?
        .as_path()
        != root
    {
        return None;
    }
    let selected = audited_registry::selected(&metadata, &snapshot.lock, &snapshot.cargo_home)?;
    let mut bindings = BTreeMap::new();
    for package in packages {
        let matching = metadata
            .packages
            .iter()
            .filter(|item| {
                item.source.is_none()
                    && std::fs::canonicalize(&item.manifest_path).ok().as_ref()
                        == Some(&package.manifest)
            })
            .collect::<Vec<_>>();
        let [owner] = matching.as_slice() else {
            return None;
        };
        let node = metadata.node(&owner.id)?;
        let mut admitted = BTreeSet::new();
        for dependency in &package.dependencies {
            if !dependency.default_registry || !dependency.audited_macro_surface_enabled() {
                continue;
            }
            let kind = match dependency.kind {
                DependencyKind::Normal => None,
                DependencyKind::Dev => Some("dev"),
                DependencyKind::Build => Some("build"),
            };
            let alias = normalize_crate_name(&dependency.key);
            if selected_dependency(node, &alias, kind, dependency.target.as_deref()).is_some_and(
                |id| {
                    selected
                        .get(&dependency.package)
                        .is_some_and(|expected| expected == id)
                },
            ) {
                admitted.insert((
                    dependency.key.clone(),
                    dependency.kind,
                    dependency.target.clone(),
                ));
            }
        }
        bindings.insert(package.manifest.clone(), admitted);
    }
    Some(bindings)
}

#[cfg(test)]
mod tests;
