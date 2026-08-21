use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePackage {
    pub name: String,
    pub path: String,
    pub dependencies: Vec<String>,
}

/// Blocking counterpart to `crate::exec::run_bounded`, for the one call site
/// that is synchronous and cannot await.
///
/// Kills the child when the limit expires and reports the expiry as an error,
/// so a hung `cargo metadata` can never be mistaken for a completed one.
fn run_sync_bounded(
    cmd: &mut std::process::Command,
    limit: std::time::Duration,
    what: &str,
) -> std::io::Result<std::process::Output> {
    use std::io::Read;

    let mut child = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // The pipes must be drained concurrently with the wait loop. `cargo
    // metadata` on a real monorepo emits far more than a pipe buffer holds, and
    // a child blocked writing into a full pipe never exits -- `try_wait` would
    // then poll until the deadline and kill a process that was only waiting for
    // us to read. `Command::output`, which this replaced, drains for the same
    // reason.
    let mut out_pipe = child.stdout.take();
    let mut err_pipe = child.stderr.take();
    let out_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = out_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(pipe) = err_pipe.as_mut() {
            let _ = pipe.read_to_end(&mut buf);
        }
        buf
    });

    let join = |h: std::thread::JoinHandle<Vec<u8>>| h.join().unwrap_or_default();

    let deadline = std::time::Instant::now() + limit;
    loop {
        match child.try_wait()? {
            Some(status) => {
                return Ok(std::process::Output {
                    status,
                    stdout: join(out_reader),
                    stderr: join(err_reader),
                });
            }
            None => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    // Killing the child closes both pipes, so the readers end.
                    let _ = join(out_reader);
                    let _ = join(err_reader);
                    tracing::warn!(
                        "{} exceeded its {}s timeout and was killed",
                        what,
                        limit.as_secs()
                    );
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        format!("{} timed out after {}s", what, limit.as_secs()),
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
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
        let out = run_sync_bounded(
            std::process::Command::new("cargo")
                .current_dir(repo_dir)
                .args(["metadata", "--format-version", "1", "--no-deps"]),
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
