//! The only module that can turn a typed model command and prompt into bytes
//! delivered to a child process.
//!
//! This module is private to `exec::agent`. Sibling `exec` modules can request
//! a delivery only through [`super::deliver`], whose inputs remain
//! [`AgentCommand`] and [`ModelPrompt`]. They cannot obtain the underlying
//! `Command`, construct the prompt-byte permit, choose a formatter, or reach
//! the raw STDIN primitive.

use anyhow::{Result, bail};
use std::borrow::Cow;
use std::process::Output;
use std::time::Duration;
use tokio::process::Command;
use tracing::warn;

use super::{AgentCommand, Framing, ProviderProbeCommand};
use crate::model_prompt::ModelPrompt;

/// Capability required to expose a [`ModelPrompt`]'s rendered bytes.
///
/// The type is nameable only so `ModelPrompt` can declare its accessor. Its
/// field and its field's type are private here, and no value ever leaves this
/// module, so safe sibling code cannot construct or acquire the capability.
pub(crate) struct ModelPromptPermit(PrivatePermit);

struct PrivatePermit;

/// Deliver one typed prompt using the formatter fixed by its provider
/// constructor.
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
                "{} exceeded its {}s timeout while being fed a typed model prompt and was killed",
                what,
                limit.as_secs()
            );
            bail!("{} timed out after {}s", what, limit.as_secs())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::agy_stream_input;

    #[test]
    fn agy_framing_is_one_ndjson_line() {
        let line = agy_stream_input("a prompt with \"quotes\" and a\nnewline");
        assert_eq!(line.lines().count(), 1, "one message, one line: {line}");
        let value: serde_json::Value = serde_json::from_str(line.trim()).expect("valid NDJSON");
        assert_eq!(value["event"], "user");
        assert_eq!(
            value["message"]["content"],
            "a prompt with \"quotes\" and a\nnewline"
        );
    }
}
