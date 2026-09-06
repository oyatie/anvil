use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct PendingResource(Arc<AtomicBool>);

impl std::future::Future for PendingResource {
    type Output = Result<()>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::task::Poll::Pending
    }
}

impl Drop for PendingResource {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn synchronous_deadline_releases_owned_pending_work_before_returning() {
    let dropped = Arc::new(AtomicBool::new(false));
    let resource = PendingResource(Arc::clone(&dropped));
    let started = Instant::now();
    let error = run_sync_task(Duration::from_millis(20), "owned inert task", || resource)
        .expect_err("pending work is bounded by the same deadline");
    assert!(error.to_string().contains("owned inert task timed out"));
    assert!(
        dropped.load(Ordering::SeqCst),
        "work was detached, not cancelled"
    );
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[test]
fn exhausted_synchronous_budget_never_starts_work() {
    let started = AtomicBool::new(false);
    let error = run_sync_task(Duration::ZERO, "expired inert task", || {
        started.store(true, Ordering::SeqCst);
        async { Ok(()) }
    })
    .expect_err("startup consumes the declared budget");
    assert!(error.to_string().contains("expired inert task timed out"));
    assert!(!started.load(Ordering::SeqCst));
}

#[tokio::test]
async fn synchronous_work_has_a_private_runtime_even_inside_an_async_caller() {
    let actual = run_sync_task(Duration::from_secs(1), "inert nested runtime", || async {
        tokio::task::yield_now().await;
        Ok("complete")
    })
    .expect("synchronous callers may already be inside Tokio");
    assert_eq!(actual, "complete");
}

#[tokio::test]
async fn synchronous_capture_preserves_output_and_nonzero_status() {
    let scratch = tempfile::tempdir().expect("ordinary diff fixture");
    let before = scratch.path().join("before");
    let after = scratch.path().join("after");
    std::fs::write(&before, "before\n").expect("before fixture");
    std::fs::write(&after, "after\n").expect("after fixture");
    let mut command = std::process::Command::new("git");
    // This LF-only fixture owns its line-ending policy, including on Windows.
    command.args([
        "-c",
        "core.autocrlf=false",
        "--no-pager",
        "diff",
        "--no-index",
        "--no-ext-diff",
        "--",
    ]);
    command.arg(&before).arg(&after);
    let command = SyncNonModelCommand::checked(command).expect("finite Git admission");
    let output = run_sync_bounded(command, Duration::from_secs(2), "inert sync diff")
        .expect("nonzero process status remains an output, not a transport error");
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8(output.stdout).expect("fixture output");
    assert!(stdout.contains("-before\n+after\n"), "{stdout}");
    assert!(
        output.stderr.is_empty(),
        "unexpected Git stderr ({} bytes; first 512 escaped): {}",
        output.stderr.len(),
        String::from_utf8_lossy(&output.stderr[..output.stderr.len().min(512)]).escape_debug()
    );
}

#[cfg(unix)]
#[test]
fn synchronous_capture_bounds_an_ordinary_finite_child() {
    let mut command = std::process::Command::new("sleep");
    command.arg("1");
    let command = SyncNonModelCommand::checked(command).expect("finite sleep admission");
    let started = Instant::now();
    let error = run_sync_bounded(command, Duration::from_millis(20), "inert sync sleep")
        .expect_err("direct finite sleep exceeds its budget");
    assert!(error.to_string().contains("inert sync sleep timed out"));
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[test]
fn synchronous_capture_preserves_launch_error_context() {
    let scratch = tempfile::tempdir().expect("working directory fixture");
    let mut command = std::process::Command::new("git");
    command
        .arg("--version")
        .current_dir(scratch.path().join("absent"));
    let command = SyncNonModelCommand::checked(command).expect("finite Git admission");
    let error = run_sync_bounded(command, Duration::from_secs(2), "inert launch failure")
        .expect_err("missing working directory prevents launch");
    assert!(
        error
            .to_string()
            .contains("inert launch failure failed to run")
    );
}

#[test]
fn synchronous_capture_drains_stderr_and_observes_both_streams_eof() {
    let mut command = std::process::Command::new("git");
    command.arg("--anvil-invalid-test-option");
    let command = SyncNonModelCommand::checked(command).expect("finite Git admission");
    let output = run_sync_bounded(command, Duration::from_secs(2), "inert Git diagnostic")
        .expect("both output streams close after the finite child exits");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--anvil-invalid-test-option"));
}

#[cfg(windows)]
#[test]
fn synchronous_capture_times_out_and_terminates_an_owned_finite_windows_child() {
    const CHILD: &str = "ANVIL_INERT_CAPTURE_CHILD_DIRECTORY";
    if let Some(directory) = std::env::var_os(CHILD) {
        let directory = std::path::PathBuf::from(directory);
        std::fs::write(directory.join("started"), "inert child started").unwrap();
        std::thread::sleep(Duration::from_secs(5));
        std::fs::write(directory.join("completed"), "finite child completed").unwrap();
        return;
    }
    let scratch = tempfile::tempdir().unwrap();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.env_clear().env(CHILD, scratch.path()).args([
        "--exact",
        "exec::non_model::transport::tests::synchronous_capture_times_out_and_terminates_an_owned_finite_windows_child",
        "--nocapture",
    ]);
    // Only this owning unit test can wrap its exact inert self-test executable;
    // the production admission vocabulary and constructors remain unchanged.
    let started = Instant::now();
    let error = run_sync_bounded(
        SyncNonModelCommand(command),
        Duration::from_secs(1),
        "inert Windows child",
    )
    .unwrap_err();
    assert!(error.to_string().contains("inert Windows child timed out"));
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(
        scratch.path().join("started").is_file(),
        "native child must actually launch"
    );
    std::thread::sleep(Duration::from_secs(5));
    assert!(
        !scratch.path().join("completed").exists(),
        "kill-on-drop must prevent the finite child from completing"
    );
}
