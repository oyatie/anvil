//! A stage's declared write scope, on disk for the length of one turn.
//!
//! `pre-commit` reads `.anvil/run-scope` and refuses staged paths outside it.
//! Nothing wrote that file until this existed, so the guardrail could not fire
//! in a real checkout (#215).
//!
//! Its own module for the same reason `prompt_file` is: `chain.rs` decides which
//! model serves a stage, and a file writer is not that decision.

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The owner line, written into the declaration itself.
///
/// The hook ignores column-zero `#` lines and grants them nothing
/// (`anvil_scope_check`: `case "$anvil_prefix" in ''|'#'*) continue`), so this
/// travels IN the file rather than beside it. A sibling file would be a second
/// thing to keep in step, and the pair going out of step is a worse failure
/// than either file alone.
const OWNER_MARK: &str = "# anvil-run-scope";

/// How long a declaration may sit before it is presumed abandoned.
///
/// `create_new` is right about concurrency and was wrong about crashes: `Drop`
/// does not run for SIGKILL, an OOM kill, or a pulled plug, so one killed run
/// left a file that refused EVERY later run in that workspace, forever, with an
/// error saying another run held it. That message was true once and false from
/// then on, and the only repair was knowing to delete a dotfile.
///
/// The ceiling is derived from the table rather than picked, so it tracks the
/// chains instead of drifting from them: the longest a stage can legitimately
/// take is the sum of its tiers' timeouts, and the worst stage today is
/// `security_audit` at 2580s over five tiers. Doubling that covers the staging
/// and commit that follow the turn, and the margin is deliberately generous --
/// stealing a live run's scope is a worse error than waiting out a dead one.
fn stale_after() -> Duration {
    let worst = crate::ai_driver::Stage::ALL
        .iter()
        .map(|s| super::chain(*s).iter().map(|t| t.timeout).sum::<Duration>())
        .max()
        .unwrap_or(Duration::from_secs(3600));
    worst * 2
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// When the declaration at `path` was made, and by which process.
///
/// Falls back to the file's mtime when the owner line is absent or
/// unparseable: a declaration written by an older build, or a partial write,
/// must still be able to age out. Absent evidence of WHEN is not evidence of
/// recency (I1) -- but neither is it licence to steal the scope, so the
/// fallback still has to clear the same ceiling.
fn owner_of(path: &Path) -> (Option<u32>, Option<u64>) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    for line in text.lines() {
        let Some(rest) = line.strip_prefix(OWNER_MARK) else {
            continue;
        };
        let mut pid = None;
        let mut started = None;
        for field in rest.split_whitespace() {
            if let Some(v) = field.strip_prefix("pid=") {
                pid = v.parse().ok();
            } else if let Some(v) = field.strip_prefix("started=") {
                started = v.parse().ok();
            }
        }
        if started.is_some() {
            return (pid, started);
        }
    }
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs());
    (None, mtime)
}

/// A stage's write scope, declared on disk for the length of one turn.
///
/// Removed on drop so ordinary work outside a run sees no constraint at all --
/// the hook treats an absent declaration as "not a milestone run", which is
/// different from an empty one meaning "may write nothing".
#[must_use = "the scope is released the moment this is dropped; hold it across the commit"]
pub struct RunScope {
    path: std::path::PathBuf,
    dir_was_created: bool,
}

impl RunScope {
    pub fn declare(working_dir: &Path, writes: &[String]) -> Result<Self> {
        let dir = working_dir.join(".anvil");
        let dir_was_created = !dir.exists();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("could not create {}", dir.display()))?;
        let path = dir.join("run-scope");
        // `create_new`: a declaration already present is another run in this
        // workspace, and silently overwriting its scope would widen or narrow a
        // constraint it is relying on.
        let mut attempt = Self::create(&path);
        if matches!(&attempt, Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists) {
            let (pid, started) = owner_of(&path);
            let age = started.map(|t| Duration::from_secs(now_secs().saturating_sub(t)));
            match age {
                Some(age) if age > stale_after() => {
                    // Say it. A run that silently takes a scope from another
                    // process is indistinguishable from the defect this fixes.
                    tracing::warn!(
                        "taking over an abandoned run scope at {} -- declared {}s ago by pid \
                         {}, past the {}s ceiling. If that process is still running, its \
                         commits are no longer scoped.",
                        path.display(),
                        age.as_secs(),
                        pid.map_or_else(|| "unknown".to_string(), |p| p.to_string()),
                        stale_after().as_secs()
                    );
                    std::fs::remove_file(&path).with_context(|| {
                        format!("could not clear the stale run scope at {}", path.display())
                    })?;
                    attempt = Self::create(&path);
                }
                _ => {
                    bail!(
                        "could not declare the run scope at {}: pid {} declared it {} and it has \
                         not aged out. Another run holds this workspace; if that process is gone, \
                         it ages out after {}s.",
                        path.display(),
                        pid.map_or_else(|| "unknown".to_string(), |p| p.to_string()),
                        age.map_or_else(
                            || "at an unknown time".to_string(),
                            |a| format!("{}s ago", a.as_secs())
                        ),
                        stale_after().as_secs()
                    )
                }
            }
        }
        let mut file = attempt
            .with_context(|| format!("could not declare the run scope at {}", path.display()))?;
        use std::io::Write;
        // Column zero, and a `#`: the hook skips it and grants it nothing, so
        // this widens no scope. It is what lets the next run tell a crash from
        // a conflict.
        writeln!(
            file,
            "{OWNER_MARK} pid={} started={}",
            std::process::id(),
            now_secs()
        )
        .with_context(|| format!("could not write the run scope at {}", path.display()))?;
        for prefix in writes {
            writeln!(file, "{prefix}")
                .with_context(|| format!("could not write the run scope at {}", path.display()))?;
        }
        file.flush()
            .with_context(|| format!("could not flush the run scope at {}", path.display()))?;
        Ok(Self {
            path,
            dir_was_created,
        })
    }

    fn create(path: &Path) -> std::io::Result<std::fs::File> {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
    }
}

impl Drop for RunScope {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if self.dir_was_created {
            // Only the directory this run created, and only if nothing else
            // landed in it.
            if let Some(dir) = self.path.parent() {
                let _ = std::fs::remove_dir(dir);
            }
        }
    }
}
