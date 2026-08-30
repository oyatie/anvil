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

pub mod agent;
pub mod build_env;
pub mod inherited;
pub mod turn;
pub use agent::{Posture, agent};
pub use inherited::INHERITED;

use anyhow::{Result, bail};
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

/// Margin between agy's own `--print-timeout` and Anvil's kill for the same
/// turn, so agy ends the turn itself (exit 1, stderr says why) before Anvil
/// drops it with no output at all.
pub const AGY_PRINT_TIMEOUT_MARGIN: Duration = Duration::from_secs(30);

/// One budget for a supervised model turn, yielding BOTH deadlines.
///
/// A supervisor bounded more tightly than the work it supervises does not
/// supervise it -- it truncates it, and then reports the failure it caused. The
/// doc-parity probe handed agy `--print-timeout 120s` and wrapped it in a
/// watchdog hardcoded to 30, so a healthy call was killed at thirty seconds and
/// the gate published `Errored`, which blocks merge-queue admission. Nothing
/// related the two numbers, so nothing could notice they disagreed.
///
/// They are one value now. `supervisor()` is what bounds the turn and
/// `tool_arg()` is what the tool is told, derived from it by subtracting
/// [`AGY_PRINT_TIMEOUT_MARGIN`] -- so the tool always ends its own turn first,
/// with a message, rather than being dropped silently. The two cannot drift
/// because there is only one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisedTurn(Duration);

impl SupervisedTurn {
    /// A turn bounded at `limit`.
    pub const fn bounded_at(limit: Duration) -> Self {
        Self(limit)
    }

    /// The budget for the watchdog around the call.
    pub const fn supervisor(self) -> Duration {
        self.0
    }

    /// The `--print-timeout` argument for the tool itself, strictly inside the
    /// supervisor's budget.
    pub fn tool_arg(self) -> String {
        agy_print_timeout_arg(self.0)
    }
}

/// agy's `--print-timeout` value (Go duration syntax) for a turn Anvil bounds
/// at `limit`.
///
/// agy's default is 5m0s and fires as `Error: timeout waiting for response`
/// (exit 1) no matter how long Anvil is willing to wait; seventeen stage
/// configs and every `ExecClass::Model` spawn allowed more than that and were
/// cut off by the default anyway. Every agy spawn passes this explicitly so
/// the two deadlines agree and the default never silently wins. Never yields
/// `0s`, which agy reads as "do not wait".
pub fn agy_print_timeout_arg(limit: Duration) -> String {
    let secs = limit
        .saturating_sub(AGY_PRINT_TIMEOUT_MARGIN)
        .as_secs()
        .max(1);
    format!("{}s", secs)
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

/// Same bound, plus delivery of a payload on the child's STDIN.
///
/// # Why this lives here and not at the call site
///
/// Writing STDIN needs `spawn()` rather than `output()`, and a hand-rolled
/// spawn/wait pair is exactly where the timeout and `kill_on_drop` pairing gets
/// lost: the timeout ends up around the write, or around the wait, and the
/// child outlives both. Keeping the spawn in this module means every provider
/// path inherits the same bound and the same kill (invariant I5).
///
/// # Deadlock and EPIPE
///
/// The write and the drain of stdout/stderr run concurrently, so a child that
/// echoes its input (every provider CLI does) cannot fill the stdout pipe and
/// block us while we are still filling its stdin.
///
/// A child that exits before reading -- a usage error, an expired login --
/// gives the writer `EPIPE`. That is the child's answer, not a harness fault,
/// so the write error is dropped and the child's own output and status are
/// returned. Rust disables `SIGPIPE` at startup, so this surfaces as an
/// ordinary `io::Error` rather than a signal.
///
/// The pipe is closed once the payload is written; without that EOF the child
/// waits for more input and every call runs to the timeout.
pub async fn run_bounded_with_stdin(
    mut cmd: Command,
    stdin_payload: &str,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    use tokio::io::AsyncWriteExt;

    cmd.kill_on_drop(true);
    cmd.stdin(std::process::Stdio::piped());

    let deliver = async {
        let mut child = match cmd.spawn() {
            Ok(child) => child,
            Err(e) => bail!("{} failed to run: {}", what, e),
        };

        let pipe = child.stdin.take();
        let write = async move {
            if let Some(mut pipe) = pipe {
                let _ = pipe.write_all(stdin_payload.as_bytes()).await;
                let _ = pipe.shutdown().await;
            }
        };
        let wait = child.wait_with_output();

        let (_, waited) = tokio::join!(write, wait);
        match waited {
            Ok(output) => Ok(output),
            Err(e) => bail!("{} failed to run: {}", what, e),
        }
    };

    match tokio::time::timeout(limit, deliver).await {
        Ok(result) => result,
        Err(_) => {
            warn!(
                "{} exceeded its {}s timeout while being fed on stdin and was killed",
                what,
                limit.as_secs()
            );
            bail!("{} timed out after {}s", what, limit.as_secs())
        }
    }
}

/// Lives here rather than in one of its callers. It was defined in
/// `queue_healer` and called from `cedar_guard`, `ci_triager`, `fixer::engine`
/// and `ai_driver::router` -- which made `cedar_guard` depend on the queue
/// healer to decide what an exit status means. The rule is an execution
/// concern and belongs beside `ExecClass` and `run_bounded`.
///
/// Result policy for a model turn that edits the workspace: any non-zero exit
/// is a failed turn. Partial stdout from a process that died mid-edit is not a
/// partial repair; it is a tree in a state nobody chose. Shared with
/// `fixer::engine`, which has the same shape and failed the same way.
pub fn interpret_agy_outcome(status_success: bool, stdout: &str, stderr: &str) -> Result<String> {
    if !status_success {
        let why = stderr.trim();
        if why.is_empty() {
            bail!("agy exited non-zero with no stderr");
        }
        bail!("agy exited non-zero: {}", why);
    }
    Ok(stdout.to_string())
}

#[cfg(test)]
mod tests {
    #[test]
    fn agy_print_timeout_sits_a_margin_under_anvils_bound() {
        use super::{ExecClass, agy_print_timeout_arg};
        use std::time::Duration;
        assert_eq!(
            agy_print_timeout_arg(ExecClass::Model.timeout()),
            "570s",
            "600s Model bound minus the 30s margin"
        );
        assert_eq!(agy_print_timeout_arg(Duration::from_secs(420)), "390s");
        // Never 0s: agy reads that as "do not wait" and the turn dies at once.
        assert_eq!(agy_print_timeout_arg(Duration::from_secs(5)), "1s");
        assert_eq!(agy_print_timeout_arg(Duration::ZERO), "1s");
    }

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
