// The only module that can hand a typed prompt to a typed model command's OS
// STDIN stream.
//
// This module is private to `exec::agent`. Sibling `exec` modules can request
// a handoff only through [`super::deliver`], whose inputs remain
// [`AgentCommand`] and [`ModelPrompt`]. They cannot obtain the underlying
// `Command`, construct the prompt-byte permit, choose a formatter, or reach
// the raw STDIN primitive.
//
// Success means every prompt byte was accepted by the OS stdin stream,
// shutdown issued EOF, and the direct child status plus captured stdout and
// stderr streams were collected before the deadline.
// It does not prove that the provider consumed or parsed those bytes.
// Provider behavior and descendants remain outside this boundary.

use anyhow::{Result, bail};
use std::borrow::Cow;
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tracing::warn;

mod session;
use session::ReaderTasks;

use super::{AgentCommand, Framing, ProviderProbeCommand};
use crate::model_prompt::ModelPrompt;

/// Capability required to expose a [`ModelPrompt`]'s rendered bytes.
///
/// The type is nameable only so `ModelPrompt` can declare its accessor. Its
/// field and its field's type are private here, and no value ever leaves this
/// module, so safe sibling code cannot construct or acquire the capability.
pub(crate) struct ModelPromptPermit(PrivatePermit);

struct PrivatePermit;

/// Hand one typed prompt to OS STDIN using the formatter fixed by its provider
/// constructor.
///
/// `Ok` is evidence of a complete stream write, EOF, and a collected direct
/// child result. It is not an acknowledgement from the provider application.
pub(super) async fn deliver(
    command: AgentCommand,
    prompt: &ModelPrompt,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    let AgentCommand { command, framing } = command;
    let permit = ModelPromptPermit(PrivatePermit);
    let rendered = prompt.as_str(&permit);
    let payload = match framing {
        Framing::Plain => Cow::Borrowed(rendered),
        Framing::AgyStreamJson => Cow::Owned(agy_stream_input(rendered)),
    };
    deliver_with_stdin(command, payload.as_ref(), limit, what).await
}

/// Executes a prompt-free provider probe whose complete argv was selected by
/// the finite provider seam. This is separate from both model delivery and the
/// raw non-model runner, so neither boundary needs a provider exception.
#[expect(
    clippy::disallowed_methods,
    reason = "typed provider-probe transport owns this execution"
)]
pub(super) async fn probe(
    ProviderProbeCommand(mut command): ProviderProbeCommand,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    command.kill_on_drop(true);
    match tokio::time::timeout(limit, command.output()).await {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => bail!("{} failed to run: {}", what, error),
        Err(_) => {
            warn!(
                "{} exceeded its {}s provider-probe timeout and was killed",
                what,
                limit.as_secs()
            );
            bail!("{} timed out after {}s", what, limit.as_secs())
        }
    }
}

/// Wrap a prompt in one line of agy's finite NDJSON input protocol.
fn agy_stream_input(prompt: &str) -> String {
    let message = serde_json::json!({
        "event": "user",
        "message": { "content": prompt },
    });
    format!("{message}\n")
}

/// Raw model-STDIN primitive. It is intentionally unreachable outside this
/// private child module; callers can reach it only after both typed
/// capabilities have been supplied to [`deliver`].
#[expect(
    clippy::disallowed_methods,
    reason = "typed model transport owns this execution"
)]
async fn deliver_with_stdin(
    mut command: Command,
    stdin_payload: &str,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    command.kill_on_drop(true);
    command.stdin(std::process::Stdio::piped());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => bail!("{} failed to run: {}", what, error),
    };
    let mut pipe = child
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("{} opened no provider stdin", what))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("{} opened no provider stdout", what))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow::anyhow!("{} opened no provider stderr", what))?;

    // Drain both output pipes while writing so a verbose provider cannot
    // deadlock input delivery. Reader tasks are aborted on every adverse path:
    // a provider descendant may retain inherited output descriptors, but that
    // must neither hide a failed write nor outlive this bounded direct turn.
    let readers = ReaderTasks {
        stdout: tokio::spawn(async move {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).await.map(|_| bytes)
        }),
        stderr: tokio::spawn(async move {
            let mut bytes = Vec::new();
            stderr.read_to_end(&mut bytes).await.map(|_| bytes)
        }),
    };
    let deadline = tokio::time::Instant::now() + limit;

    let write_and_close = async move {
        let result = async {
            pipe.write_all(stdin_payload.as_bytes()).await?;
            pipe.shutdown().await
        }
        .await;
        drop(pipe);
        result
    };
    let captured =
        session::run(&mut child, write_and_close, readers, deadline, limit, what).await?;
    Ok(Output {
        status: captured.status,
        stdout: captured.stdout,
        stderr: captured.stderr,
    })
}

#[cfg(test)]
mod tests;
