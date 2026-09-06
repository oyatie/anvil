// Platform-owned asynchronous capture for the checked synchronous runner.

use anyhow::Result;
use std::process::{Output, Stdio};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};

#[cfg(any(windows, test))]
mod names;
#[cfg(windows)]
mod windows;

pub(super) struct Capture {
    #[cfg(windows)]
    stdout: tokio::net::windows::named_pipe::NamedPipeServer,
    #[cfg(windows)]
    stderr: tokio::net::windows::named_pipe::NamedPipeServer,
}

pub(super) async fn prepare(command: &mut Command) -> Result<Capture> {
    #[cfg(windows)]
    {
        // Both parent-owned clients must connect before either writer is
        // installed in the child command. Any error aborts before launch.
        let (stdout, stdout_writer) = windows::pair(names::Stream::Stdout).await?;
        let (stderr, stderr_writer) = windows::pair(names::Stream::Stderr).await?;
        command
            .stdout(Stdio::from(stdout_writer))
            .stderr(Stdio::from(stderr_writer));
        Ok(Capture { stdout, stderr })
    }
    #[cfg(not(windows))]
    {
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        Ok(Capture {})
    }
}

impl Capture {
    pub(super) async fn finish(self, mut child: Child) -> Result<Output> {
        #[cfg(windows)]
        let (mut stdout, mut stderr) = (self.stdout, self.stderr);
        #[cfg(not(windows))]
        let (mut stdout, mut stderr) = (
            child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("captured stdout is absent"))?,
            child
                .stderr
                .take()
                .ok_or_else(|| anyhow::anyhow!("captured stderr is absent"))?,
        );
        let mut stdout_bytes = Vec::new();
        let mut stderr_bytes = Vec::new();
        let wait = async {
            loop {
                if let Some(status) = child.try_wait()? {
                    return Ok::<_, std::io::Error>(status);
                }
                // Do not register Tokio's Windows process-wait callback:
                // its teardown synchronously unregisters that callback.
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        };
        let (status, _, _) = tokio::try_join!(
            wait,
            stdout.read_to_end(&mut stdout_bytes),
            stderr.read_to_end(&mut stderr_bytes),
        )?;
        Ok(Output {
            status,
            stdout: stdout_bytes,
            stderr: stderr_bytes,
        })
    }
}
