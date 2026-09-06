use super::PackageManifest;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Eq, PartialEq)]
pub(super) struct Snapshot {
    root: PathBuf,
    invocation: PathBuf,
    pub(super) cargo_home: PathBuf,
    pub(super) rustup_home: PathBuf,
    pub(super) lock: String,
    manifests: Vec<(PathBuf, Vec<u8>)>,
    environment: Vec<(&'static str, Option<OsString>)>,
}

impl Snapshot {
    pub(super) fn read(root: &Path, packages: &[PackageManifest]) -> Option<Self> {
        let invocation = fs::canonicalize(std::env::current_dir().ok()?).ok()?;
        let cargo_home = tool_home(
            std::env::var_os("CARGO_HOME"),
            std::env::var_os("HOME"),
            std::env::var_os("USERPROFILE"),
            ".cargo",
        )?;
        let cargo_home = fs::canonicalize(cargo_home).ok()?;
        let rustup_home = tool_home(
            std::env::var_os("RUSTUP_HOME"),
            std::env::var_os("HOME"),
            std::env::var_os("USERPROFILE"),
            ".rustup",
        )?;
        let rustup_home = fs::canonicalize(rustup_home).ok()?;
        Self::read_at(
            root,
            packages,
            invocation,
            cargo_home,
            rustup_home,
            crate::exec::build_env::BUILD_INHERITED
                .iter()
                .map(|name| (*name, std::env::var_os(name)))
                .collect(),
        )
    }

    pub(super) fn read_at(
        root: &Path,
        packages: &[PackageManifest],
        invocation: PathBuf,
        cargo_home: PathBuf,
        rustup_home: PathBuf,
        environment: Vec<(&'static str, Option<OsString>)>,
    ) -> Option<Self> {
        let directories = packages
            .iter()
            .map(|package| package.directory.clone())
            .chain([root.to_owned(), invocation.clone()])
            .collect::<Vec<_>>();
        if !configuration_absent(&directories, &cargo_home) {
            return None;
        }
        let paths = packages
            .iter()
            .map(|package| package.manifest.clone())
            .chain([root.join("Cargo.toml")])
            .collect::<BTreeSet<_>>();
        let mut manifests = Vec::new();
        for path in paths {
            let canonical = fs::canonicalize(&path).ok()?;
            if !canonical.starts_with(root) || canonical != path {
                return None;
            }
            let bytes = fs::read(&path).ok()?;
            let value: toml::Value = std::str::from_utf8(&bytes).ok()?.parse().ok()?;
            if !supported_manifest(&value)
                || packages
                    .iter()
                    .any(|package| package.manifest == path && package.value != value)
            {
                return None;
            }
            manifests.push((path, bytes));
        }
        let lock_path = fs::canonicalize(root.join("Cargo.lock")).ok()?;
        if !lock_path.starts_with(root) {
            return None;
        }
        Some(Self {
            root: root.to_owned(),
            invocation,
            cargo_home,
            rustup_home,
            lock: fs::read_to_string(lock_path).ok()?,
            manifests,
            environment,
        })
    }
}

pub(super) fn tool_home(
    explicit: Option<OsString>,
    home: Option<OsString>,
    user_profile: Option<OsString>,
    leaf: &str,
) -> Option<PathBuf> {
    let path = match explicit {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(home.or(user_profile)?).join(leaf),
    };
    path.is_absolute().then_some(path)
}

pub(super) fn configuration_absent(directories: &[PathBuf], cargo_home: &Path) -> bool {
    let configs = directories
        .iter()
        .flat_map(|directory| {
            directory
                .ancestors()
                .map(|ancestor| ancestor.join(".cargo"))
        })
        .chain([cargo_home.to_owned()])
        .collect::<BTreeSet<_>>();
    configs.iter().all(|directory| {
        match fs::symlink_metadata(directory) {
            Ok(metadata) if !metadata.is_dir() || metadata.file_type().is_symlink() => return false,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
            Err(_) => return false,
            Ok(_) => {}
        }
        ["config", "config.toml"].iter().all(|name| matches!(fs::symlink_metadata(directory.join(name)), Err(error) if error.kind() == std::io::ErrorKind::NotFound))
    })
}

pub(super) fn supported_manifest(value: &toml::Value) -> bool {
    if value.get("patch").is_some() || value.get("replace").is_some() {
        return false;
    }
    fn supported(value: &toml::Value) -> bool {
        match value {
            toml::Value::Table(table) => {
                !table.contains_key("registry")
                    && !table.contains_key("git")
                    && table.values().all(supported)
            }
            toml::Value::Array(values) => values.iter().all(supported),
            _ => true,
        }
    }
    supported(value)
}
