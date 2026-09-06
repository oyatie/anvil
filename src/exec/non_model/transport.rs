// Execution primitives reachable only after the parent module has minted a
// checked non-model capability.

use anyhow::{Result, bail};
use std::process::{ExitStatus, Output};
use std::time::{Duration, Instant};
use tracing::warn;

use super::{NonModelCommand, SyncNonModelCommand};
use crate::exec::ExecClass;

mod sync_capture;

#[expect(
    clippy::disallowed_methods,
    reason = "checked non-model transport owns this execution"
)]
pub(super) async fn run_for(
    NonModelCommand(mut command): NonModelCommand,
    limit: Duration,
    what: &str,
    class: Option<ExecClass>,
) -> Result<Output> {
    command.kill_on_drop(true);
    match tokio::time::timeout(limit, command.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => bail!("{} failed to run: {}", what, error),
        Err(_) => {
            if let Some(class) = class {
                warn!(
                    "{} exceeded the {} timeout of {}s and was killed",
                    what,
                    class.label(),
                    limit.as_secs()
                );
                bail!(
                    "{} timed out after {}s ({} class)",
                    what,
                    limit.as_secs(),
                    class.label()
                )
            }
            warn!(
                "{} exceeded its {}s timeout and was killed",
                what,
                limit.as_secs()
            );
            bail!("{} timed out after {}s", what, limit.as_secs())
        }
    }
}

#[expect(
    clippy::disallowed_methods,
    reason = "checked non-model transport owns this execution"
)]
pub(super) async fn run_status(
    NonModelCommand(mut command): NonModelCommand,
    what: &str,
) -> Result<ExitStatus> {
    command.kill_on_drop(true);
    command.stdin(std::process::Stdio::null());
    command
        .status()
        .await
        .map_err(|error| anyhow::anyhow!("{} failed to run: {}", what, error))
}

#[expect(
    clippy::disallowed_methods,
    reason = "checked non-model transport owns this execution"
)]
pub(super) async fn run_with_stdin(
    NonModelCommand(mut command): NonModelCommand,
    stdin_payload: &str,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    use tokio::io::AsyncWriteExt;

    command.kill_on_drop(true);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let deliver = async {
        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => bail!("{} failed to run: {}", what, error),
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
            Err(error) => bail!("{} failed to run: {}", what, error),
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

#[expect(
    clippy::disallowed_methods,
    reason = "checked synchronous non-model transport owns this execution"
)]
pub(super) fn run_sync_bounded(
    SyncNonModelCommand(command): SyncNonModelCommand,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    run_sync_task(limit, what, || async move {
        // Conversion preserves the checked canonical program, argv and
        // environment. Async pipe ownership lets deadline cancellation close
        // both streams rather than waiting for blocking readers to see EOF.
        let mut command = tokio::process::Command::from(command);
        command
            .kill_on_drop(true)
            .stdin(std::process::Stdio::null());
        let capture = sync_capture::prepare(&mut command)
            .await
            .map_err(|error| anyhow::anyhow!("{} capture setup failed: {}", what, error))?;
        let child = command
            .spawn()
            .map_err(|error| anyhow::anyhow!("{} failed to run: {}", what, error))?;
        // Stdio::from(File) retains parent writer copies in Command. They
        // must close immediately so only child-held writers govern EOF.
        drop(command);
        capture
            .finish(child)
            .await
            .map_err(|error| anyhow::anyhow!("{} failed while waiting: {}", what, error))
    })
}

fn run_sync_task<T, F>(limit: Duration, what: &str, task: impl FnOnce() -> F + Send) -> Result<T>
where
    T: Send,
    F: std::future::Future<Output = Result<T>>,
{
    // Thread/runtime startup consumes this same budget. No task is started
    // after it expires, and no blocking reader or blocking-pool task is used.
    let deadline = Instant::now()
        .checked_add(limit)
        .ok_or_else(|| anyhow::anyhow!("{} timeout exceeds the clock range", what))?;
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("anvil-sync-exec".to_owned())
            .spawn_scoped(scope, move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| anyhow::anyhow!("{} runtime failed: {}", what, error))?;
                runtime.block_on(async {
                    if Instant::now() >= deadline {
                        bail!("{} timed out after {}s", what, limit.as_secs());
                    }
                    match tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), task())
                        .await
                    {
                        Ok(result) => result,
                        Err(_) => bail!("{} timed out after {}s", what, limit.as_secs()),
                    }
                })
            })
            .map_err(|error| anyhow::anyhow!("{} worker failed to start: {}", what, error))?
            .join()
            .map_err(|_| anyhow::anyhow!("{} worker panicked", what))?
    })
}

#[cfg(test)]
mod tests;
