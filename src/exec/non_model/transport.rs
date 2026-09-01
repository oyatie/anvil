//! Execution primitives reachable only after the parent module has minted a
//! checked non-model capability.

use anyhow::{Result, bail};
use std::process::{ExitStatus, Output};
use std::time::{Duration, Instant};
use tracing::warn;

use super::{NonModelCommand, SyncNonModelCommand};
use crate::exec::ExecClass;

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

pub(super) async fn run_status(
    NonModelCommand(mut command): NonModelCommand,
    what: &str,
) -> Result<ExitStatus> {
    command.kill_on_drop(true);
    command
        .status()
        .await
        .map_err(|error| anyhow::anyhow!("{} failed to run: {}", what, error))
}

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

pub(super) fn run_sync_bounded(
    SyncNonModelCommand(mut command): SyncNonModelCommand,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    use std::io::Read;

    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| anyhow::anyhow!("{} failed to run: {}", what, error))?;
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let stdout_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(pipe) = stdout.as_mut() {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(pipe) = stderr.as_mut() {
            let _ = pipe.read_to_end(&mut bytes);
        }
        bytes
    });

    let deadline = Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                bail!("{} timed out after {}s", what, limit.as_secs());
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                bail!("{} failed while waiting: {}", what, error);
            }
        }
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow::anyhow!("{} stdout reader panicked", what))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow::anyhow!("{} stderr reader panicked", what))?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
