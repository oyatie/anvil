//! Bounded subprocess execution.
//!
//! Anvil spawns 118 subprocesses across the tree. Before this module exactly one
//! of them had a timeout, and one had `kill_on_drop`. `ModelExecutionConfig`
//! carried a `print_timeout_secs` field that was set in 23 places and read
//! nowhere.
//!
//! Two consequences, both observed in production logs:
//!   - a hung provider CLI pinned its task, its per-PR mutex and a pipe buffer
//!     indefinitely, with nothing to reclaim them;
//!   - `tokio::task::JoinHandle::abort()` drops a `Child` without killing it, so
//!     cancelled work left orphaned `agy`, `gh` and `cargo` processes behind.
//!
//! Invariant I5: every subprocess has a timeout AND `kill_on_drop(true)`.

use anyhow::{bail, Result};
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tracing::warn;

/// How long a class of subprocess may run before it is killed.
///
/// Expressed as a class rather than a bare number so call sites state intent,
/// and so the bounds can be tuned in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecClass {
    /// Local metadata queries: `git rev-parse`, `git status`, `which`.
    Quick,
    /// Network round-trips to GitHub: `gh api`, `gh pr view`.
    Api,
    /// Repository mutations: `git clone`, `git fetch`, `git worktree add`.
    Vcs,
    /// Build and test invocations: `cargo check`, `cargo test`, `npm test`.
    Build,
    /// Model inference through a provider CLI.
    Model,
}

impl ExecClass {
    pub const fn timeout(self) -> Duration {
        match self {
            ExecClass::Quick => Duration::from_secs(30),
            ExecClass::Api => Duration::from_secs(60),
            ExecClass::Vcs => Duration::from_secs(300),
            ExecClass::Build => Duration::from_secs(1_800),
            ExecClass::Model => Duration::from_secs(600),
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            ExecClass::Quick => "quick",
            ExecClass::Api => "api",
            ExecClass::Vcs => "vcs",
            ExecClass::Build => "build",
            ExecClass::Model => "model",
        }
    }
}

/// Runs a command to completion under a class-appropriate timeout.
///
/// Sets `kill_on_drop(true)` so cancelling the surrounding task reaps the child
/// instead of orphaning it. On timeout the future is dropped, which triggers the
/// kill, and an error is returned rather than a partial `Output` -- a timed-out
/// process produced no measurement, and must not be mistaken for one (I1).
///
/// Known limitation: `kill_on_drop` reaps only the direct child. `gh`, `agy`,
/// `cursor-agent` and `cargo` all fork helpers of their own, which survive.
/// Full containment needs a process group and a negative-pgid kill; that is
/// tracked separately and is not attempted here.
pub async fn run_bounded(mut cmd: Command, class: ExecClass, what: &str) -> Result<Output> {
    cmd.kill_on_drop(true);
    match tokio::time::timeout(class.timeout(), cmd.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => bail!("{} failed to run: {}", what, e),
        Err(_) => {
            warn!(
                "{} exceeded the {} timeout of {}s and was killed",
                what,
                class.label(),
                class.timeout().as_secs()
            );
            bail!(
                "{} timed out after {}s ({} class)",
                what,
                class.timeout().as_secs(),
                class.label()
            )
        }
    }
}

/// Same bound, with an explicit duration for callers that carry their own
/// configured limit (for example `ModelExecutionConfig::print_timeout_secs`).
pub async fn run_bounded_for(mut cmd: Command, limit: Duration, what: &str) -> Result<Output> {
    cmd.kill_on_drop(true);
    match tokio::time::timeout(limit, cmd.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => bail!("{} failed to run: {}", what, e),
        Err(_) => {
            warn!(
                "{} exceeded its {}s timeout and was killed",
                what,
                limit.as_secs()
            );
            bail!("{} timed out after {}s", what, limit.as_secs())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_output_for_a_fast_command() {
        let mut c = Command::new("echo");
        c.arg("hello");
        let out = run_bounded(c, ExecClass::Quick, "echo").await.expect("ok");
        assert!(String::from_utf8_lossy(&out.stdout).contains("hello"));
    }

    #[tokio::test]
    async fn a_hung_command_is_killed_and_reported_as_an_error() {
        let mut c = Command::new("sleep");
        c.arg("30");
        let err = run_bounded_for(c, Duration::from_millis(200), "sleep")
            .await
            .expect_err("must time out");
        let msg = err.to_string();
        assert!(msg.contains("timed out"), "unexpected: {msg}");
    }

    #[tokio::test]
    async fn a_missing_binary_is_an_error_not_a_silent_pass() {
        let c = Command::new("anvil-no-such-binary-xyz");
        let err = run_bounded(c, ExecClass::Quick, "probe")
            .await
            .expect_err("must error");
        assert!(err.to_string().contains("failed to run"));
    }

    #[test]
    fn timeouts_are_ordered_by_expected_cost() {
        assert!(ExecClass::Quick.timeout() < ExecClass::Api.timeout());
        assert!(ExecClass::Api.timeout() < ExecClass::Vcs.timeout());
        assert!(ExecClass::Vcs.timeout() < ExecClass::Model.timeout());
        assert!(ExecClass::Model.timeout() < ExecClass::Build.timeout());
    }
}
