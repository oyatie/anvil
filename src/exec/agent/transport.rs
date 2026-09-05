//! The only module that can hand a typed prompt to a typed model command's OS
//! STDIN stream.
//!
//! This module is private to `exec::agent`. Sibling `exec` modules can request
//! a handoff only through [`super::deliver`], whose inputs remain
//! [`AgentCommand`] and [`ModelPrompt`]. They cannot obtain the underlying
//! `Command`, construct the prompt-byte permit, choose a formatter, or reach
//! the raw STDIN primitive.
//!
//! Success means every prompt byte was accepted by the OS stdin stream,
//! shutdown issued EOF, and the direct child status plus captured stdout and
//! stderr streams were collected before the deadline.
//! It does not prove that the provider consumed or parsed those bytes.
//! Provider behavior and descendants remain outside this boundary.

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
    let mut readers = ReaderTasks {
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

    let handed_off = tokio::time::timeout_at(deadline, async {
        pipe.write_all(stdin_payload.as_bytes()).await?;
        pipe.shutdown().await
    })
    .await;
    drop(pipe);
    match handed_off {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            terminate(&mut child).await;
            bail!(
                "{} failed to write the complete typed model prompt to its OS stdin stream: {}",
                what,
                error
            );
        }
        Err(_) => {
            terminate(&mut child).await;
            return child_timed_out(what, limit);
        }
    }

    let status = match tokio::time::timeout_at(deadline, child.wait()).await {
        Ok(Ok(status)) => status,
        Ok(Err(error)) => {
            terminate(&mut child).await;
            bail!("{} failed to run: {}", what, error);
        }
        Err(_) => {
            terminate(&mut child).await;
            return child_timed_out(what, limit);
        }
    };

    let reads = tokio::time::timeout_at(deadline, readers.finish(what)).await;
    let (stdout, stderr) = match reads {
        Ok(result) => result?,
        Err(_) => return captured_streams_timed_out(what, limit),
    };
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

async fn terminate(child: &mut tokio::process::Child) {
    let _ = child.start_kill();
    let _ = child.wait().await;
}

type ReadTask = tokio::task::JoinHandle<std::io::Result<Vec<u8>>>;

struct ReaderTasks {
    stdout: ReadTask,
    stderr: ReadTask,
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

fn child_timed_out(what: &str, limit: Duration) -> Result<Output> {
    warn!(
        "{} direct model child exceeded its {}s turn deadline and was killed",
        what,
        limit.as_secs()
    );
    bail!("{} timed out after {}s", what, limit.as_secs())
}

fn captured_streams_timed_out(what: &str, limit: Duration) -> Result<Output> {
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

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_BYTE_READER_TEST: &str =
        "exec::agent::transport::tests::one_byte_reader_native_child";

    fn bounded_prompt(diff_frames: usize) -> ModelPrompt {
        let chunk = "x".repeat(crate::reviewer::untrusted::MAX_DIFF_CHARS);
        let mut builder = ModelPrompt::builder();
        for _ in 0..diff_frames {
            builder.push_untrusted(crate::reviewer::untrusted::Untrusted::new(
                crate::reviewer::untrusted::UntrustedLabel::GitDiff,
                &chunk,
            ));
        }
        builder
            .finish_for(crate::model_prompt::ModelPromptPurpose::SubscriptionProbe)
            .expect("bounded provider prompt")
    }

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

    #[cfg(unix)]
    #[tokio::test]
    async fn os_stdin_handoff_preserves_exact_prompt_bytes_and_eof() {
        let prompt = bounded_prompt(2);
        let expected = prompt
            .as_str(&ModelPromptPermit(PrivatePermit))
            .as_bytes()
            .to_vec();
        let command = AgentCommand {
            command: Command::new("/bin/cat"),
            framing: Framing::Plain,
        };

        let output = deliver(command, &prompt, Duration::from_secs(10), "echo provider")
            .await
            .expect("the OS accepted every byte and EOF before cat exited");
        assert!(output.status.success());
        assert_eq!(output.stdout, expected);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_large_direct_shebang_handoff_reports_an_observed_incomplete_write() {
        let executable = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/direct_prefix_provider.sh");
        assert!(executable.is_file(), "missing direct shebang fixture");

        let command = AgentCommand {
            command: Command::new(executable),
            framing: Framing::Plain,
        };
        let prompt = bounded_prompt(2);
        let error = deliver(command, &prompt, Duration::from_secs(10), "prefix reader")
            .await
            .expect_err("the OS reported that the direct shebang reader closed stdin early");
        assert!(
            error.to_string().contains("OS stdin stream"),
            "unexpected handoff error: {error}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_large_native_handoff_reports_an_observed_incomplete_write() {
        let mut raw = Command::new(std::env::current_exe().expect("current test executable"));
        raw.args(["--exact", ONE_BYTE_READER_TEST, "--ignored", "--nocapture"])
            .env("ANVIL_ONE_BYTE_READER_MODE", "large");
        let command = AgentCommand {
            command: raw,
            framing: Framing::Plain,
        };

        let error = deliver(
            command,
            &bounded_prompt(2),
            Duration::from_secs(10),
            "prefix reader",
        )
        .await
        .expect_err("the OS reported that the native reader closed stdin early");
        assert!(
            error.to_string().contains("OS stdin stream"),
            "unexpected handoff error: {error}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_buffered_os_handoff_is_not_a_provider_consumption_claim() {
        let mut raw = Command::new(std::env::current_exe().expect("current test executable"));
        raw.args(["--exact", ONE_BYTE_READER_TEST, "--ignored", "--nocapture"])
            .env("ANVIL_ONE_BYTE_READER_MODE", "buffered");
        let command = AgentCommand {
            command: raw,
            framing: Framing::Plain,
        };

        let output = deliver(
            command,
            &bounded_prompt(0),
            Duration::from_secs(10),
            "buffered one-byte reader",
        )
        .await
        .expect("a small payload can be fully accepted by the OS before the child reads it");
        assert!(output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("READ_ONE_BYTE"),
            "fixture did not confirm its one-byte application read"
        );
    }

    #[test]
    #[ignore = "private subprocess fixture for one-byte provider reads"]
    fn one_byte_reader_native_child() {
        let mode =
            std::env::var("ANVIL_ONE_BYTE_READER_MODE").expect("one-byte reader mode is present");
        assert!(matches!(mode.as_str(), "large" | "buffered"));
        if mode == "buffered" {
            std::thread::sleep(Duration::from_millis(100));
        }
        let mut byte = [0_u8; 1];
        std::io::Read::read_exact(&mut std::io::stdin(), &mut byte).expect("read one prompt byte");
        print!("READ_ONE_BYTE\nAPPROVE");
    }

    #[tokio::test]
    async fn dropping_reader_tasks_aborts_both_readers() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };

        struct Dropped(Arc<AtomicBool>);
        impl Drop for Dropped {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        fn pending_reader(dropped: Arc<AtomicBool>, started: Arc<AtomicBool>) -> ReadTask {
            tokio::spawn(async move {
                let _dropped = Dropped(dropped);
                started.store(true, Ordering::SeqCst);
                std::future::pending::<std::io::Result<Vec<u8>>>().await
            })
        }

        let stdout_dropped = Arc::new(AtomicBool::new(false));
        let stderr_dropped = Arc::new(AtomicBool::new(false));
        let stdout_started = Arc::new(AtomicBool::new(false));
        let stderr_started = Arc::new(AtomicBool::new(false));
        let readers = ReaderTasks {
            stdout: pending_reader(Arc::clone(&stdout_dropped), Arc::clone(&stdout_started)),
            stderr: pending_reader(Arc::clone(&stderr_dropped), Arc::clone(&stderr_started)),
        };
        tokio::time::timeout(Duration::from_secs(1), async {
            while !stdout_started.load(Ordering::SeqCst) || !stderr_started.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("both reader tasks started");
        drop(readers);
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if stdout_dropped.load(Ordering::SeqCst) && stderr_dropped.load(Ordering::SeqCst) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("both reader tasks were aborted");
        assert!(stdout_dropped.load(Ordering::SeqCst));
        assert!(stderr_dropped.load(Ordering::SeqCst));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn post_exit_inherited_stream_timeout_does_not_claim_the_child_was_killed() {
        let mut raw = Command::new("/bin/sh");
        raw.args(["-c", "cat >/dev/null; sleep 1 & printf APPROVE; exit 0"]);
        let command = AgentCommand {
            command: raw,
            framing: Framing::Plain,
        };

        let started = std::time::Instant::now();
        let error = deliver(
            command,
            &bounded_prompt(0),
            Duration::from_millis(250),
            "provider with inherited output",
        )
        .await
        .expect_err("inherited output streams must remain bounded after the child exits");
        let message = error.to_string();
        assert!(message.contains("direct child exited"), "{message}");
        assert!(!message.contains("killed"), "{message}");
        assert!(
            started.elapsed() < Duration::from_millis(750),
            "reader timeout waited on an excluded descendant"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn an_observed_stdin_write_failure_is_not_masked_by_descendant_output_handles() {
        let mut raw = Command::new("/bin/sh");
        raw.args(["-c", "exec 0<&-; sleep 1 & printf APPROVE; exit 0"]);
        let command = AgentCommand {
            command: raw,
            framing: Framing::Plain,
        };

        let started = std::time::Instant::now();
        let error = deliver(
            command,
            &bounded_prompt(2),
            Duration::from_millis(250),
            "prefix reader with inherited output",
        )
        .await
        .expect_err("inherited output handles cannot mask an OS stdin write failure");
        assert!(
            error.to_string().contains("OS stdin stream"),
            "unexpected handoff error: {error}"
        );
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "delivery failure waited on an unrelated descendant"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn missing_provider_binary_is_absent_evidence() {
        let command = AgentCommand {
            command: Command::new("/definitely/not/an/anvil-provider"),
            framing: Framing::Plain,
        };
        let error = deliver(
            command,
            &bounded_prompt(1),
            Duration::from_secs(2),
            "missing provider",
        )
        .await
        .expect_err("a missing provider cannot yield a model verdict");
        assert!(error.to_string().contains("failed to run"), "{error}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn hung_provider_is_bounded() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = tempfile::tempdir().expect("provider fixture");
        let executable = fixture.path().join("provider");
        std::fs::write(&executable, "#!/bin/sh\nwhile :; do :; done\n")
            .expect("write provider fixture");
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
            .expect("executable provider fixture");
        let command = AgentCommand {
            command: Command::new(executable),
            framing: Framing::Plain,
        };

        let error = deliver(
            command,
            &bounded_prompt(1),
            Duration::from_millis(300),
            "hung provider",
        )
        .await
        .expect_err("a hung provider cannot outlive the direct-child bound");
        assert!(error.to_string().contains("timed out"), "{error}");
    }
}
