use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use super::{PackageManifest, TargetKind};

impl PackageManifest {
    pub(in crate::source_scan::paths::module_graph::roots) fn crate_aliases(
        &self,
        target_kind: TargetKind,
        packages: &[PackageManifest],
    ) -> Result<BTreeSet<String>, String> {
        Ok(self
            .local_alias_targets(target_kind, packages)?
            .into_keys()
            .collect())
    }

    /// Preserve each resolved destination instead of mistaking equal names
    /// for equal crates. Multiple conditional destinations remain ambiguous.
    pub(in crate::source_scan::paths::module_graph::roots) fn local_alias_targets(
        &self,
        target_kind: TargetKind,
        packages: &[PackageManifest],
    ) -> Result<BTreeMap<String, BTreeSet<PathBuf>>, String> {
        let mut aliases: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();
        if target_kind == TargetKind::Binary
            && let Some(name) = self.library_name()?
        {
            aliases
                .entry(name)
                .or_default()
                .insert(self.directory.clone());
        }
        for dependency in &self.dependencies {
            if !dependency.kind.visible_to(target_kind) {
                continue;
            }
            let directories = match &dependency.directory {
                Some(directory) => vec![directory],
                None => self
                    .global_overrides
                    .iter()
                    .filter(|candidate| candidate.package == dependency.package)
                    .map(|candidate| &candidate.directory)
                    .collect(),
            };
            for directory in directories {
                let directory = fs::canonicalize(directory).map_err(|error| {
                    format!("cannot resolve dependency {}: {error}", directory.display())
                })?;
                let target = packages
                    .iter()
                    .find(|package| package.directory == directory)
                    .ok_or_else(|| {
                        format!(
                            "local dependency {} was not discovered",
                            directory.display()
                        )
                    })?;
                let alias = if dependency.renamed {
                    normalize_crate_name(&dependency.key)
                } else {
                    target.library_name()?.ok_or_else(|| {
                        format!(
                            "local dependency {} has no library target",
                            target.manifest.display()
                        )
                    })?
                };
                aliases.entry(alias).or_default().insert(directory);
            }
        }
        Ok(aliases)
    }

    fn library_name(&self) -> Result<Option<String>, String> {
        let package = self
            .value
            .get("package")
            .and_then(toml::Value::as_table)
            .ok_or_else(|| format!("{} has no [package] table", self.manifest.display()))?;
        let library = self.value.get("lib").and_then(toml::Value::as_table);
        let has_library = library.is_some()
            || (package
                .get("autolib")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true)
                && self.directory.join("src/lib.rs").is_file());
        if !has_library {
            return Ok(None);
        }
        let name = library
            .and_then(|table| table.get("name"))
            .and_then(toml::Value::as_str)
            .or_else(|| package.get("name").and_then(toml::Value::as_str))
            .ok_or_else(|| format!("{} has no package/library name", self.manifest.display()))?;
        Ok(Some(normalize_crate_name(name)))
    }
}

pub(in crate::source_scan::paths::module_graph::roots) fn normalize_crate_name(
    name: &str,
) -> String {
    name.replace('-', "_")
}
