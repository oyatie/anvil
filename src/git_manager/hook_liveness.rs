//! Whether a checkout's git hooks will actually run.
//!
//! Installing a hook and having a hook are different facts, and git reports
//! the difference nowhere. Two ways to hold a checkout that runs nothing:
//!
//!   * `core.hooksPath` set to a directory that does not exist. Git runs no
//!     hooks and prints nothing. Measured on this repository:
//!     `core.hooksPath = <repo>/.githooks`, a path retired long ago.
//!   * the hooks renamed aside. The same checkout held
//!     `pre-commit.stale-untracked.bak`, `.bak2`, and no `pre-commit`.
//!
//! Both were live at once, and every local rung -- pre-commit, commit-msg,
//! pre-push -- can be dead while a test reads only the installer.
//! stayed green throughout: it asserts the templates are tracked and that the
//! installer's SOURCE does not set the retired path. Neither question is
//! "does this checkout run hooks", so neither could catch it.
//!
//! Reported as findings rather than a bool, because "not installed" and
//! "installed but dangling" call for different actions: a fresh clone or a CI
//! runner legitimately has no hooks, while a dangling `core.hooksPath` is
//! always a defect.

use std::path::{Path, PathBuf};
use std::process::Command;

/// One way a checkout fails to run the hooks it appears to have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDefect {
    /// `core.hooksPath` names a directory that is not there. Always a defect:
    /// git silently runs nothing, and no fresh clone produces this state.
    DanglingHooksPath { configured: String },
    /// The hook is absent from the effective directory.
    Missing { hook: String, looked_in: PathBuf },
    /// Present but not executable, so git skips it without complaint.
    NotExecutable { hook: String, path: PathBuf },
    /// Present but not what the tracked template says. A hook edited in place
    /// is a hook nobody reviewed.
    DriftedFromTemplate { hook: String, path: PathBuf },
    /// The template itself is missing from the tree, so nothing can be
    /// installed or compared. Never silently a pass.
    TemplateAbsent { hook: String, expected: PathBuf },
}

impl HookDefect {
    /// Whether this defect means the checkout is actively lying about being
    /// governed, as opposed to simply not having been set up yet.
    pub fn is_always_a_defect(&self) -> bool {
        !matches!(self, HookDefect::Missing { .. })
    }
}

/// The directory git will actually look in, and whether the configured
/// override resolves.
pub fn effective_hooks_dir(repo: &Path) -> (Option<PathBuf>, Option<String>) {
    let configured = git_out(repo, &["config", "--get", "core.hooksPath"]);
    if let Some(p) = configured.as_deref().filter(|p| !p.is_empty()) {
        let path = if Path::new(p).is_absolute() {
            PathBuf::from(p)
        } else {
            repo.join(p)
        };
        return if path.is_dir() {
            (Some(path), None)
        } else {
            (None, Some(p.to_string()))
        };
    }
    let common = git_out(repo, &["rev-parse", "--git-common-dir"])
        .filter(|s| !s.is_empty())
        .map(|s| {
            let p = PathBuf::from(&s);
            if p.is_absolute() { p } else { repo.join(p) }
        });
    (common.map(|c| c.join("hooks")), None)
}

/// Every way this checkout would fail to run `hooks`, given templates under
/// `template_dir`.
pub fn defects(repo: &Path, template_dir: &Path, hooks: &[&str]) -> Vec<HookDefect> {
    let mut out = Vec::new();
    let (dir, dangling) = effective_hooks_dir(repo);
    if let Some(configured) = dangling {
        out.push(HookDefect::DanglingHooksPath { configured });
        return out; // nothing below can be true while git looks nowhere
    }
    let Some(dir) = dir else {
        return out;
    };
    for hook in hooks {
        let tmpl = template_dir.join(hook);
        let Ok(want) = std::fs::read(&tmpl) else {
            out.push(HookDefect::TemplateAbsent {
                hook: (*hook).to_string(),
                expected: tmpl,
            });
            continue;
        };
        let path = dir.join(hook);
        let Ok(got) = std::fs::read(&path) else {
            out.push(HookDefect::Missing {
                hook: (*hook).to_string(),
                looked_in: dir.clone(),
            });
            continue;
        };
        if !is_executable(&path) {
            out.push(HookDefect::NotExecutable {
                hook: (*hook).to_string(),
                path: path.clone(),
            });
        }
        if got != want {
            out.push(HookDefect::DriftedFromTemplate {
                hook: (*hook).to_string(),
                path,
            });
        }
    }
    out
}

fn is_executable(p: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(p).is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        p.is_file()
    }
}

fn git_out(repo: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.args(args).current_dir(repo);
    let out = crate::exec::run_sync_bounded(
        command,
        crate::exec::ExecClass::Quick.timeout(),
        "git hook liveness query",
    )
    .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}
