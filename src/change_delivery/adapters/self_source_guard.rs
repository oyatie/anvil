//! No lane may share a git repository with the tree the daemon runs from.
//! A worktree of the daemon's own repo shares its object store and refs;
//! mutating it from an autonomous lane is mutating the running system.

use std::path::Path;

pub async fn git_toplevel(dir: &Path) -> Option<String> {
    let mut cmd = tokio::process::Command::new("git");
    cmd.current_dir(dir).args(["rev-parse", "--show-toplevel"]);
    let out = crate::exec::run_bounded(
        cmd,
        crate::exec::ExecClass::Quick,
        "git rev-parse (lane guard)",
    )
    .await
    .ok()?;
    if !out.status.success() {
        return None;
    }
    let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
    std::fs::canonicalize(&raw)
        .map(|p| p.display().to_string())
        .ok()
        .or(Some(raw))
}

/// `Err(reason)` when `repo_dir` is the same git repository the daemon's
/// working directory belongs to.
pub async fn assert_not_daemon_tree(repo_dir: &Path) -> Result<(), String> {
    let daemon_cwd = std::env::current_dir().map_err(|e| e.to_string())?;
    let (Some(lane_top), Some(daemon_top)) = (
        git_toplevel(repo_dir).await,
        git_toplevel(&daemon_cwd).await,
    ) else {
        return Ok(());
    };
    if lane_top == daemon_top {
        return Err(format!(
            "lane repository {lane_top} is the daemon's own source tree; lanes for {} must use the managed clone under repos/",
            repo_dir.display()
        ));
    }
    Ok(())
}
