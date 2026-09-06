use super::*;

const ONE_BYTE_READER_TEST: &str = "exec::agent::transport::tests::one_byte_reader_native_child";

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
