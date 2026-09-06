use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::TargetKind;

#[derive(Clone)]
pub(super) struct DependencySpec {
    pub(super) key: String,
    pub(super) package: String,
    pub(super) renamed: bool,
    pub(super) directory: Option<PathBuf>,
    pub(super) kind: DependencyKind,
    pub(super) target: Option<String>,
    pub(super) default_registry: bool,
    pub(super) default_features: bool,
    pub(super) features: BTreeSet<String>,
}

#[derive(Clone)]
pub(super) struct LocalOverride {
    pub(super) package: String,
    pub(super) directory: PathBuf,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum DependencyKind {
    Normal,
    Dev,
    Build,
}

impl DependencySpec {
    pub(super) fn audited_macro_surface_enabled(&self) -> bool {
        if !self.default_features {
            return false;
        }
        match self.package.as_str() {
            "serde" => exact_features(&self.features, &[&["derive"]]),
            "clap" => exact_features(&self.features, &[&["derive"], &["derive", "env"]]),
            "tokio" => exact_features(
                &self.features,
                &[&["full"], &["macros", "rt"], &["macros", "rt-multi-thread"]],
            ),
            "async-trait" | "tracing" | "anyhow" | "serde_json" => self.features.is_empty(),
            _ => false,
        }
    }
}

fn exact_features(actual: &BTreeSet<String>, allowed: &[&[&str]]) -> bool {
    allowed
        .iter()
        .any(|set| actual.len() == set.len() && set.iter().all(|feature| actual.contains(*feature)))
}

impl DependencyKind {
    pub(super) fn visible_to(self, target: TargetKind) -> bool {
        matches!(
            (self, target),
            (Self::Normal, TargetKind::Library | TargetKind::Binary)
                | (Self::Build, TargetKind::BuildScript)
        )
    }
}

pub(super) fn collect(
    manifest: &toml::Value,
    inherited: Option<&toml::map::Map<String, toml::Value>>,
    workspace_root: &Path,
    package_dir: &Path,
) -> Result<Vec<DependencySpec>, String> {
    let mut tables = Vec::new();
    let manifest_table = manifest
        .as_table()
        .ok_or_else(|| "Cargo manifest root is not a table".to_owned())?;
    add_tables(manifest_table, None, &mut tables);
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for (condition, target) in targets {
            if let Some(target) = target.as_table() {
                add_tables(target, Some(condition.as_str()), &mut tables);
            }
        }
    }
    let mut dependencies = Vec::new();
    for (table, kind, target) in tables {
        for (name, specification) in table {
            let mut dependency = parse_spec(
                name,
                specification,
                kind,
                inherited,
                workspace_root,
                package_dir,
            )?;
            dependency.target = target.map(str::to_owned);
            dependencies.push(dependency);
        }
    }
    Ok(dependencies)
}

fn add_tables<'a>(
    manifest: &'a toml::map::Map<String, toml::Value>,
    target: Option<&'a str>,
    tables: &mut Vec<(
        &'a toml::map::Map<String, toml::Value>,
        DependencyKind,
        Option<&'a str>,
    )>,
) {
    for (name, kind) in [
        ("dependencies", DependencyKind::Normal),
        ("dev-dependencies", DependencyKind::Dev),
        ("build-dependencies", DependencyKind::Build),
    ] {
        if let Some(table) = manifest.get(name).and_then(toml::Value::as_table) {
            tables.push((table, kind, target));
        }
    }
}

fn parse_spec(
    name: &str,
    specification: &toml::Value,
    kind: DependencyKind,
    inherited: Option<&toml::map::Map<String, toml::Value>>,
    workspace_root: &Path,
    package_dir: &Path,
) -> Result<DependencySpec, String> {
    let direct = specification.as_table();
    let direct_features = features(direct)?;
    let (specification, base) = if direct
        .and_then(|table| table.get("workspace"))
        .and_then(toml::Value::as_bool)
        == Some(true)
    {
        let direct = direct.expect("workspace dependency table");
        if direct.contains_key("path") || direct.contains_key("package") {
            return Err(format!(
                "dependency `{name}` combines `workspace = true` with local authority"
            ));
        }
        let inherited = inherited
            .and_then(|table| table.get(name))
            .ok_or_else(|| format!("workspace dependency `{name}` cannot be resolved"))?;
        (inherited, workspace_root)
    } else {
        (specification, package_dir)
    };
    let table = specification.as_table();
    let mut enabled_features = features(table)?;
    enabled_features.extend(direct_features);
    if table.is_none() && !specification.is_str() {
        return Err(format!(
            "dependency `{name}` has an unsupported specification"
        ));
    }
    let package = table
        .and_then(|table| table.get("package"))
        .and_then(toml::Value::as_str)
        .unwrap_or(name)
        .to_owned();
    let renamed = table.is_some_and(|table| table.contains_key("package"));
    let directory = table
        .and_then(|table| table.get("path"))
        .map(|path| {
            path.as_str()
                .map(|path| base.join(path))
                .ok_or_else(|| format!("dependency `{name}` path is not a string"))
        })
        .transpose()?;
    let default_registry = directory.is_none()
        && table.is_none_or(|table| {
            table.get("version").is_some_and(toml::Value::is_str)
                && !table.contains_key("git")
                && !table.contains_key("registry")
        });
    let default_features = table
        .and_then(|table| table.get("default-features"))
        .and_then(toml::Value::as_bool)
        != Some(false)
        && direct
            .and_then(|table| table.get("default-features"))
            .and_then(toml::Value::as_bool)
            != Some(false);
    Ok(DependencySpec {
        key: name.to_owned(),
        package,
        renamed,
        directory,
        kind,
        target: None,
        default_registry,
        default_features,
        features: enabled_features,
    })
}

fn features(
    table: Option<&toml::map::Map<String, toml::Value>>,
) -> Result<BTreeSet<String>, String> {
    let Some(values) = table.and_then(|table| table.get("features")) else {
        return Ok(BTreeSet::new());
    };
    values
        .as_array()
        .ok_or_else(|| "dependency features must be an array".to_owned())?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "dependency feature must be a string".to_owned())
        })
        .collect()
}

pub(super) fn overrides(manifest: &toml::Value, root: &Path) -> Result<Vec<LocalOverride>, String> {
    let mut overrides = Vec::new();
    if let Some(registries) = manifest.get("patch").and_then(toml::Value::as_table) {
        for registry in registries.values() {
            let table = registry
                .as_table()
                .ok_or_else(|| "[patch] registry entry must be a table".to_owned())?;
            collect_overrides(table, root, &mut overrides)?;
        }
    }
    if let Some(replacements) = manifest.get("replace").and_then(toml::Value::as_table) {
        collect_overrides(replacements, root, &mut overrides)?;
    }
    Ok(overrides)
}

fn collect_overrides(
    table: &toml::map::Map<String, toml::Value>,
    root: &Path,
    overrides: &mut Vec<LocalOverride>,
) -> Result<(), String> {
    for (key, value) in table {
        let Some(specification) = value.as_table() else {
            continue;
        };
        let Some(path) = specification.get("path") else {
            continue;
        };
        let path = path
            .as_str()
            .ok_or_else(|| format!("override `{key}` path is not a string"))?;
        let package = specification
            .get("package")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| key.split(':').next().unwrap_or(key));
        overrides.push(LocalOverride {
            package: package.to_owned(),
            directory: root.join(path),
        });
    }
    Ok(())
}
