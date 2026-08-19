use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePackage {
    pub name: String,
    pub path: String,
    pub dependencies: Vec<String>,
}

pub struct WorkspaceDagSelector;

impl WorkspaceDagSelector {
    pub fn new() -> Self {
        Self
    }

    /// 100% Deterministic calculation of affected workspace packages from modified file paths
    pub fn select_affected_packages(
        &self,
        changed_files: &[String],
        packages: &[WorkspacePackage],
    ) -> Vec<String> {
        let mut directly_affected = Vec::new();

        for pkg in packages {
            if changed_files.iter().any(|f| f.starts_with(&pkg.path) || f.contains(&pkg.name)) {
                directly_affected.push(pkg.name.clone());
            }
        }

        // Compute transitive dependents
        let mut all_affected = directly_affected.clone();
        let mut changed = true;
        while changed {
            changed = false;
            for pkg in packages {
                if !all_affected.contains(&pkg.name) {
                    if pkg.dependencies.iter().any(|dep| all_affected.contains(dep)) {
                        all_affected.push(pkg.name.clone());
                        changed = true;
                    }
                }
            }
        }

        all_affected
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
