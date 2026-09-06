//! Prepared direct-turn resources only; spawning and typed prompt access stay
//! in the parent transport. Tests control events without starting processes.

use anyhow::{Result, bail};
use std::future::Future;
use std::io;
use std::time::Duration;
use tokio::time::Instant;
use tracing::warn;

pub(super) trait DirectChild {
    type Status;
    fn wait(&mut self) -> impl Future<Output = io::Result<Self::Status>> + Send;
    fn terminate_and_reap(&mut self) -> impl Future<Output = ()> + Send;
}

impl DirectChild for tokio::process::Child {
    type Status = std::process::ExitStatus;

    async fn wait(&mut self) -> io::Result<Self::Status> {
        tokio::process::Child::wait(self).await
    }

    async fn terminate_and_reap(&mut self) {
        let _ = self.start_kill();
        let _ = tokio::process::Child::wait(self).await;
    }
}

pub(super) struct Captured<S> {
    pub status: S,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// One deadline governs handoff, direct-child exit, and captured output.
/// The write future owns stdin: completion or cancellation drops it before
/// cleanup. Every adverse path drops the owned reader guard without awaiting EOF.
pub(super) async fn run<C: DirectChild, W>(
    child: &mut C,
    write_and_close: W,
    readers: ReaderTasks,
    deadline: Instant,
    limit: Duration,
    what: &str,
) -> Result<Captured<C::Status>>
where
    W: Future<Output = io::Result<()>>,
{
    match tokio::time::timeout_at(deadline, write_and_close).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            child.terminate_and_reap().await;
            bail!(
                "{} failed to write the complete typed model prompt to its OS stdin stream: {}",
                what,
                error
            );
        }
        Err(_) => {
            child.terminate_and_reap().await;
            return child_timed_out(what, limit);
        }
    }
    let status = match tokio::time::timeout_at(deadline, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            child.terminate_and_reap().await;
            bail!("{} failed to run: {}", what, error);
        }
        Err(_) => {
            child.terminate_and_reap().await;
            return child_timed_out(what, limit);
        }
    };
    let (stdout, stderr) = collect_after_exit(readers, deadline, limit, what).await?;
    Ok(Captured {
        status,
        stdout,
        stderr,
    })
}

/// Child exit was observed by the caller. This phase has no child cleanup
/// capability and can only cancel its owned readers on failure.
async fn collect_after_exit(
    mut readers: ReaderTasks,
    deadline: Instant,
    limit: Duration,
    what: &str,
) -> Result<(Vec<u8>, Vec<u8>)> {
    match tokio::time::timeout_at(deadline, readers.finish(what)).await {
        Ok(result) => result,
        Err(_) => {
            warn!(
                "{}'s direct child exited, but inherited stdout/stderr streams remained open past {:?}; reader tasks were aborted",
                what, limit
            );
            bail!(
                "{} timed out after {:?} waiting for captured streams after its direct child exited",
                what,
                limit
            )
        }
    }
}

pub(super) type ReadTask = tokio::task::JoinHandle<io::Result<Vec<u8>>>;

pub(super) struct ReaderTasks {
    pub stdout: ReadTask,
    pub stderr: ReadTask,
}

impl ReaderTasks {
    async fn finish(&mut self, what: &str) -> Result<(Vec<u8>, Vec<u8>)> {
        let stdout = (&mut self.stdout)
            .await
            .map_err(|error| anyhow::anyhow!("{} stdout reader failed: {}", what, error))?
            .map_err(|error| anyhow::anyhow!("{} stdout read failed: {}", what, error))?;
        let stderr = (&mut self.stderr)
            .await
            .map_err(|error| anyhow::anyhow!("{} stderr reader failed: {}", what, error))?
            .map_err(|error| anyhow::anyhow!("{} stderr read failed: {}", what, error))?;
        Ok((stdout, stderr))
    }
}

impl Drop for ReaderTasks {
    fn drop(&mut self) {
        self.stdout.abort();
        self.stderr.abort();
    }
}

fn child_timed_out<T>(what: &str, limit: Duration) -> Result<T> {
    warn!(
        "{} direct model child exceeded its {:?} turn deadline and was killed",
        what, limit
    );
    bail!("{} timed out after {:?}", what, limit)
}

#[cfg(test)]
mod tests;
