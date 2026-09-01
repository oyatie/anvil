use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

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
    pub fn discover_workspace_packages_sync(repo_dir: &Path) -> Vec<WorkspacePackage> {
        let cargo_toml = repo_dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Vec::new();
        }

        // This call site is synchronous all the way up to
        // `webhook::pipelines::review`, so the async `crate::exec::run_bounded`
        // helper cannot be applied without making the whole chain async. The
        // bound is therefore enforced here by hand, using the same class
        // duration the async twin below gets, and the child is killed rather
        // than left running when it expires.
        let mut command = std::process::Command::new("cargo");
        command
            .current_dir(repo_dir)
            .args(["metadata", "--format-version", "1", "--no-deps"]);
        let out = crate::exec::run_sync_bounded(
            command,
            crate::exec::ExecClass::Build.timeout(),
            "cargo metadata --no-deps (sync)",
        );

        if let Ok(output) = out
            && output.status.success()
            && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
            && let Some(packages) = val.get("packages").and_then(|p| p.as_array())
        {
            let mut res = Vec::new();
            for pkg in packages {
                let name = pkg
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let manifest = pkg
                    .get("manifest_path")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");
                let path = if let Some(parent) = Path::new(manifest).parent() {
                    parent
                        .strip_prefix(repo_dir)
                        .unwrap_or(parent)
                        .to_string_lossy()
                        .to_string()
                } else {
                    String::new()
                };

                let mut deps = Vec::new();
                if let Some(dep_array) = pkg.get("dependencies").and_then(|d| d.as_array()) {
                    for d in dep_array {
                        if let Some(dname) = d.get("name").and_then(|n| n.as_str()) {
                            deps.push(dname.to_string());
                        }
                    }
                }

                res.push(WorkspacePackage {
                    name,
                    path,
                    dependencies: deps,
                });
            }
            return res;
        }

        Vec::new()
    }

    /// Dynamically loads workspace packages from `cargo metadata` if present
    pub async fn discover_workspace_packages(repo_dir: &Path) -> Vec<WorkspacePackage> {
        let cargo_toml = repo_dir.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Vec::new();
        }

        let mut meta_cmd = Command::new("cargo");
        meta_cmd
            .current_dir(repo_dir)
            .args(["metadata", "--format-version", "1", "--no-deps"]);
        let out = crate::exec::run_bounded(
            meta_cmd,
            crate::exec::ExecClass::Build,
            "cargo metadata --no-deps",
        )
        .await;

        if let Ok(output) = out
            && output.status.success()
            && let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout)
            && let Some(packages) = val.get("packages").and_then(|p| p.as_array())
        {
            let mut res = Vec::new();
            for pkg in packages {
                let name = pkg
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let manifest = pkg
                    .get("manifest_path")
                    .and_then(|m| m.as_str())
                    .unwrap_or("");
                let path = if let Some(parent) = Path::new(manifest).parent() {
                    parent
                        .strip_prefix(repo_dir)
                        .unwrap_or(parent)
                        .to_string_lossy()
                        .to_string()
                } else {
                    String::new()
                };

                let mut deps = Vec::new();
                if let Some(dep_array) = pkg.get("dependencies").and_then(|d| d.as_array()) {
                    for d in dep_array {
                        if let Some(dname) = d.get("name").and_then(|n| n.as_str()) {
                            deps.push(dname.to_string());
                        }
                    }
                }

                res.push(WorkspacePackage {
                    name,
                    path,
                    dependencies: deps,
                });
            }
            return res;
        }

        Vec::new()
    }

    /// 100% Deterministic calculation of affected workspace packages from modified file paths
    pub fn select_affected_packages(
        &self,
        changed_files: &[String],
        packages: &[WorkspacePackage],
    ) -> Vec<String> {
        let mut directly_affected = Vec::new();

        for pkg in packages {
            if changed_files.iter().any(|f| {
                (!pkg.path.is_empty() && f.starts_with(&pkg.path)) || f.contains(&pkg.name)
            }) {
                directly_affected.push(pkg.name.clone());
            }
        }

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
}
