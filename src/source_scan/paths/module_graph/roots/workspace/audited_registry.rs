// Archive-backed source identity for the finite reviewed macro/support closure.
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::authority::metadata::Metadata;
mod pins;
#[cfg(test)]
mod tests;

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

pub(super) fn selected(
    metadata: &Metadata,
    lock: &str,
    cargo_home: &Path,
) -> Option<BTreeMap<String, String>> {
    let lock: toml::Value = lock.parse().ok()?;
    let records = lock.get("package")?.as_array()?;
    let source_base = fs::canonicalize(cargo_home.join("registry/src")).ok()?;
    let mut admitted = BTreeMap::new();
    for &(name, version, archive, fingerprint) in pins::PACKAGES {
        let matching = metadata
            .packages
            .iter()
            .filter(|package| package.name == name && package.version == version)
            .collect::<Vec<_>>();
        let [package] = matching.as_slice() else {
            return None;
        };
        if package.source.as_deref() != Some(REGISTRY) {
            return None;
        }
        let locked = records
            .iter()
            .filter(|record| {
                record.get("name").and_then(toml::Value::as_str) == Some(name)
                    && record.get("version").and_then(toml::Value::as_str) == Some(version)
            })
            .collect::<Vec<_>>();
        let [locked] = locked.as_slice() else {
            return None;
        };
        if locked.get("source").and_then(toml::Value::as_str) != Some(REGISTRY)
            || locked.get("checksum").and_then(toml::Value::as_str) != Some(archive)
        {
            return None;
        }
        let manifest = canonical_manifest(&package.manifest_path, &source_base)?;
        let directory = manifest.parent()?;
        if !directory.starts_with(&source_base) || source_fingerprint(directory)? != fingerprint {
            return None;
        }
        let node = metadata.node(&package.id)?;
        if matches!(name, "clap" | "clap_builder" | "clap_derive")
            && node
                .features
                .iter()
                .any(|feature| feature == "unstable-markdown")
        {
            return None;
        }
        admitted.insert(name.to_owned(), package.id.clone());
    }
    // The selected support edges, not just coexisting package names, must use
    // the reviewed implementations. Runtime-only dependencies are not macro grants.
    for id in admitted.values() {
        if !metadata
            .node(id)?
            .reviewed_edges_match(&admitted, &metadata.packages)
        {
            return None;
        }
    }
    Some(admitted)
}

fn canonical_manifest(path: &Path, cache: &Path) -> Option<PathBuf> {
    if !path.is_absolute() || path.file_name()? != "Cargo.toml" {
        return None;
    }
    // Cargo's native Windows paths need not carry canonicalize's verbatim
    // prefix. Compare canonical identity, while rejecting actual link entries.
    for ancestor in path.ancestors() {
        if fs::symlink_metadata(ancestor)
            .ok()?
            .file_type()
            .is_symlink()
        {
            return None;
        }
    }
    let canonical = fs::canonicalize(path).ok()?;
    (canonical.starts_with(cache) && canonical.is_file()).then_some(canonical)
}

fn source_fingerprint(root: &Path) -> Option<String> {
    let canonical = fs::canonicalize(root).ok()?;
    if canonical != root || fs::symlink_metadata(root).ok()?.file_type().is_symlink() {
        return None;
    }
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files)?;
    let mut digest = Sha256::new();
    for (relative, file_digest) in files {
        digest.update(relative.as_bytes());
        digest.update(b"\0");
        digest.update(file_digest.as_bytes());
        digest.update(b"\n");
    }
    Some(hex::encode(digest.finalize()))
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, String>,
) -> Option<()> {
    for entry in fs::read_dir(directory).ok()? {
        let path: PathBuf = entry.ok()?.path();
        let kind = fs::symlink_metadata(&path).ok()?.file_type();
        if kind.is_symlink() {
            return None;
        }
        let canonical = fs::canonicalize(&path).ok()?;
        if !canonical.starts_with(root) || canonical != path {
            return None;
        }
        if kind.is_dir() {
            collect_files(root, &path, files)?;
        } else if kind.is_file() {
            if path == root.join(".cargo-ok") {
                continue;
            }
            let relative = path
                .strip_prefix(root)
                .ok()?
                .components()
                .map(|component| {
                    let std::path::Component::Normal(name) = component else {
                        return None;
                    };
                    let name = name.to_str()?;
                    (!name.contains(['/', '\\'])).then_some(name)
                })
                .collect::<Option<Vec<_>>>()?
                .join("/");
            let hash = hex::encode(Sha256::digest(fs::read(&path).ok()?));
            if files.insert(relative, hash).is_some() {
                return None;
            }
        } else {
            return None;
        }
    }
    Some(())
}
