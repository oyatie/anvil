use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePackage {
    pub name: String,
    pub path: String,
    pub dependencies: Vec<String>,
}

pub struct WorkspaceDagSelector;

impl Default for WorkspaceDagSelector {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceDagSelector {
    pub fn new() -> Self {
        Self
    }

    /// Dynamically loads workspace packages from `cargo metadata` synchronously if present
    pub fn discover_workspace_packages_sync(repo_dir: &Path) -> Result<Vec<WorkspacePackage>> {
        let cargo_toml = repo_dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Ok(Vec::new());
        }

        let repo = repo_dir.to_path_buf();
        std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("build isolated metadata runtime")?
                .block_on(Self::discover_workspace_packages(&repo))
        })
        .join()
        .map_err(|_| anyhow::anyhow!("isolated metadata runtime panicked"))?
    }

    /// Dynamically loads workspace packages from `cargo metadata` if present
    pub async fn discover_workspace_packages(repo_dir: &Path) -> Result<Vec<WorkspacePackage>> {
        let cargo_toml = repo_dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Ok(Vec::new());
        }

        let mut meta_cmd = crate::exec::build_env::command("cargo");
        meta_cmd.current_dir(repo_dir).args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
        ]);
        let output = crate::exec::run_bounded(
            meta_cmd,
            crate::exec::ExecClass::Build,
            "cargo metadata --no-deps",
        )
        .await
        .context("cargo metadata did not run")?;
        if !output.status.success() {
            bail!(
                "cargo metadata failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        parse_metadata(&output.stdout)
    }

    /// 100% Deterministic calculation of affected workspace packages from modified file paths
    pub fn select_affected_packages(
        &self,
        changed_files: &[String],
        packages: &[WorkspacePackage],
    ) -> Vec<String> {
        let mut directly_affected = Vec::new();
        let root = packages.iter().find(|package| package.path == ".");

        for file in changed_files {
            if workspace_wide(file) {
                directly_affected.extend(packages.iter().map(|package| package.name.clone()));
                continue;
            }
            let file = Path::new(file);
            let mut owners = packages
                .iter()
                .filter(|package| package.path != "." && file.starts_with(&package.path))
                .collect::<Vec<_>>();
            if let Some(longest) = owners
                .iter()
                .map(|package| Path::new(&package.path).components().count())
                .max()
            {
                owners.retain(|package| Path::new(&package.path).components().count() == longest);
                directly_affected.extend(owners.into_iter().map(|package| package.name.clone()));
            } else if let Some(root) = root.filter(|_| known_root_package_input(file)) {
                directly_affected.push(root.name.clone());
            } else {
                // A changed path with no owning package is workspace policy or
                // an unfamiliar build input. Sparing packages would be a
                // guess, so select the whole measured workspace.
                directly_affected.extend(packages.iter().map(|package| package.name.clone()));
            }
        }
        directly_affected.sort();
        directly_affected.dedup();

        // Compute transitive dependents
        let mut all_affected = directly_affected.clone();
        let mut changed = true;
        while changed {
            changed = false;
            for pkg in packages {
                if !all_affected.contains(&pkg.name)
                    && pkg
                        .dependencies
                        .iter()
                        .any(|dep| all_affected.contains(dep))
                {
                    all_affected.push(pkg.name.clone());
                    changed = true;
                }
            }
        }

        all_affected
    }

    /// Computes the target pruning ratio: 1.0 - (selected / total)
    pub fn calculate_pruning_ratio(selected_count: usize, total_count: usize) -> f64 {
        if total_count == 0 {
            return 0.0;
        }
        let ratio = 1.0 - (selected_count as f64 / total_count as f64);
        ratio.clamp(0.0, 1.0)
    }
}

fn known_root_package_input(path: &Path) -> bool {
    if path == Path::new("build.rs") {
        return true;
    }
    matches!(
        path.components().next(),
        Some(std::path::Component::Normal(component))
            if matches!(component.to_str(), Some("src" | "tests" | "examples" | "benches"))
    )
}

fn parse_metadata(bytes: &[u8]) -> Result<Vec<WorkspacePackage>> {
    let value: serde_json::Value = serde_json::from_slice(bytes).context("parse cargo metadata")?;
    let workspace = value
        .get("workspace_root")
        .and_then(|root| root.as_str())
        .map(Path::new)
        .context("cargo metadata omitted workspace_root")?;
    let packages = value
        .get("packages")
        .and_then(|packages| packages.as_array())
        .context("cargo metadata omitted packages")?;
    packages
        .iter()
        .map(|package| {
            let name = package
                .get("name")
                .and_then(|name| name.as_str())
                .context("cargo package omitted name")?;
            let manifest = package
                .get("manifest_path")
                .and_then(|path| path.as_str())
                .map(Path::new)
                .context("cargo package omitted manifest_path")?;
            let parent = manifest.parent().context("manifest has no parent")?;
            let dependencies = package
                .get("dependencies")
                .and_then(|dependencies| dependencies.as_array())
                .into_iter()
                .flatten()
                .filter_map(|dependency| dependency.get("name").and_then(|name| name.as_str()))
                .map(str::to_owned)
                .collect();
            Ok(WorkspacePackage {
                name: name.to_owned(),
                path: package_path(parent, workspace),
                dependencies,
            })
        })
        .collect()
}

fn package_path(manifest_parent: &Path, workspace: &Path) -> String {
    let relative = manifest_parent
        .strip_prefix(workspace)
        .unwrap_or(manifest_parent);
    if relative.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        relative.to_string_lossy().replace('\\', "/")
    }
}

fn workspace_wide(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    matches!(
        normalized.as_str(),
        "Cargo.toml" | "Cargo.lock" | "rust-toolchain" | "rust-toolchain.toml"
    ) || normalized.starts_with(".cargo/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selects_affected_and_dependents() {
        let dag = WorkspaceDagSelector::new();
        let pkgs = vec![
            WorkspacePackage {
                name: "core".to_string(),
                path: "crates/core".to_string(),
                dependencies: vec![],
            },
            WorkspacePackage {
                name: "api".to_string(),
                path: "crates/api".to_string(),
                dependencies: vec!["core".to_string()],
            },
            WorkspacePackage {
                name: "unrelated".to_string(),
                path: "crates/unrelated".to_string(),
                dependencies: vec![],
            },
        ];

        let changed = vec!["crates/core/src/lib.rs".to_string()];
        let affected = dag.select_affected_packages(&changed, &pkgs);

        assert!(affected.contains(&"core".to_string()));
        assert!(affected.contains(&"api".to_string()));
        assert!(!affected.contains(&"unrelated".to_string()));
    }

    #[test]
    fn root_and_workspace_owned_changes_never_select_an_empty_set() {
        let packages = vec![
            WorkspacePackage {
                name: "root".to_owned(),
                path: ".".to_owned(),
                dependencies: vec![],
            },
            WorkspacePackage {
                name: "child".to_owned(),
                path: "crates/child".to_owned(),
                dependencies: vec!["root".to_owned()],
            },
        ];
        let selector = WorkspaceDagSelector::new();
        assert_eq!(
            selector.select_affected_packages(&["src/lib.rs".to_owned()], &packages),
            vec!["root".to_owned(), "child".to_owned()]
        );
        let mut all = selector.select_affected_packages(&["Cargo.lock".to_owned()], &packages);
        all.sort();
        assert_eq!(all, vec!["child".to_owned(), "root".to_owned()]);
    }

    #[test]
    fn unfamiliar_root_inputs_select_independent_workspace_members() {
        let packages = vec![
            WorkspacePackage {
                name: "root".to_owned(),
                path: ".".to_owned(),
                dependencies: vec![],
            },
            WorkspacePackage {
                name: "independent".to_owned(),
                path: "crates/independent".to_owned(),
                dependencies: vec![],
            },
        ];
        let selector = WorkspaceDagSelector::new();
        for changed in ["ci/policy.toml", "deny.toml", "scripts/codegen.rs"] {
            let mut affected = selector.select_affected_packages(&[changed.to_owned()], &packages);
            affected.sort();
            assert_eq!(
                affected,
                ["independent".to_owned(), "root".to_owned()],
                "unknown workspace input {changed} spared an independent package"
            );
        }
        assert_eq!(
            selector.select_affected_packages(&["src/lib.rs".to_owned()], &packages),
            ["root".to_owned()],
            "a conventional root-package source should retain precise ownership"
        );
    }

    #[test]
    fn package_ownership_uses_path_components() {
        let packages = vec![
            WorkspacePackage {
                name: "core".to_owned(),
                path: "crates/core".to_owned(),
                dependencies: vec![],
            },
            WorkspacePackage {
                name: "core-extra".to_owned(),
                path: "crates/core-extra".to_owned(),
                dependencies: vec![],
            },
        ];
        assert_eq!(
            WorkspaceDagSelector::new()
                .select_affected_packages(&["crates/core-extra/src/lib.rs".to_owned()], &packages),
            vec!["core-extra".to_owned()]
        );
    }
}
