use super::*;
use tokio::sync::oneshot;

const SAFETY_BOUND: Duration = Duration::from_secs(5);

async fn bounded<F: Future>(future: F) -> F::Output {
    tokio::time::timeout(SAFETY_BOUND, future)
        .await
        .expect("controlled session must complete without releasing pending readers")
}

struct DropSignal(Option<oneshot::Sender<()>>);

impl Drop for DropSignal {
    fn drop(&mut self) {
        if let Some(signal) = self.0.take() {
            let _ = signal.send(());
        }
    }
}

async fn pending_reader() -> (ReadTask, oneshot::Sender<()>, oneshot::Receiver<()>) {
    let (started, ready) = oneshot::channel();
    let (release, blocked) = oneshot::channel();
    let (dropped, observed_drop) = oneshot::channel();
    let task = tokio::spawn(async move {
        let _signal = DropSignal(Some(dropped));
        let _ = started.send(());
        let _ = blocked.await;
        Ok(Vec::new())
    });
    bounded(ready).await.expect("reader started");
    (task, release, observed_drop)
}

async fn pending_readers() -> (
    ReaderTasks,
    [oneshot::Sender<()>; 2],
    [oneshot::Receiver<()>; 2],
) {
    let (stdout, release_out, dropped_out) = pending_reader().await;
    let (stderr, release_err, dropped_err) = pending_reader().await;
    (
        ReaderTasks { stdout, stderr },
        [release_out, release_err],
        [dropped_out, dropped_err],
    )
}

async fn both_aborted(drops: [oneshot::Receiver<()>; 2], releases: [oneshot::Sender<()>; 2]) {
    // Keep both senders alive and never release completion: only cancellation
    // can drop these started readers. No elapsed-time comparison is the proof.
    for dropped in drops {
        bounded(dropped).await.expect("reader guard cancelled task");
    }
    assert!(releases.iter().all(oneshot::Sender::is_closed));
}

enum Wait {
    Exit,
    Error,
}

struct Child {
    outcome: Wait,
    waits: usize,
    cleanups: usize,
    writer_dropped: Option<oneshot::Receiver<()>>,
}

impl Child {
    fn new(outcome: Wait) -> Self {
        Self {
            outcome,
            waits: 0,
            cleanups: 0,
            writer_dropped: None,
        }
    }
}

impl DirectChild for Child {
    type Status = &'static str;

    async fn wait(&mut self) -> io::Result<Self::Status> {
        self.waits += 1;
        match self.outcome {
            Wait::Exit => Ok("direct child exited"),
            Wait::Error => Err(io::Error::other("wait failure")),
        }
    }

    async fn terminate_and_reap(&mut self) {
        if let Some(dropped) = &mut self.writer_dropped {
            dropped
                .try_recv()
                .expect("writer resource dropped before cleanup");
        }
        self.cleanups += 1;
    }
}

#[tokio::test]
async fn observed_write_error_does_not_wait_for_inherited_readers() {
    let (readers, releases, drops) = pending_readers().await;
    let mut child = Child::new(Wait::Exit);
    let error = bounded(run(
        &mut child,
        std::future::ready(Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "closed stdin",
        ))),
        readers,
        Instant::now() + SAFETY_BOUND,
        SAFETY_BOUND,
        "controlled provider",
    ))
    .await
    .err()
    .expect("observed write failure");
    assert!(error.to_string().contains("OS stdin stream"), "{error}");
    assert!(!error.to_string().contains("timed out"), "{error}");
    assert_eq!(child.waits, 0);
    assert_eq!(child.cleanups, 1);
    both_aborted(drops, releases).await;
}

#[tokio::test]
async fn an_expired_handoff_is_not_relabelled_as_an_observed_write_error() {
    let (readers, releases, drops) = pending_readers().await;
    let mut child = Child::new(Wait::Exit);
    let (dropped, observed_drop) = oneshot::channel();
    child.writer_dropped = Some(observed_drop);
    let writer_resource = DropSignal(Some(dropped));
    let write = async move {
        let _resource = writer_resource;
        std::future::pending::<io::Result<()>>().await
    };
    let error = bounded(run(
        &mut child,
        write,
        readers,
        Instant::now(),
        Duration::from_millis(250),
        "controlled provider",
    ))
    .await
    .err()
    .expect("expired handoff");
    assert!(
        error.to_string().contains("timed out after 250ms"),
        "{error}"
    );
    assert!(!error.to_string().contains("OS stdin stream"), "{error}");
    assert_eq!(child.waits, 0);
    assert_eq!(child.cleanups, 1);
    both_aborted(drops, releases).await;
}

#[tokio::test]
async fn successful_handoff_waits_for_the_child_and_collects_both_streams() {
    let mut child = Child::new(Wait::Exit);
    let readers = ReaderTasks {
        stdout: tokio::spawn(async { Ok(b"stdout".to_vec()) }),
        stderr: tokio::spawn(async { Ok(b"stderr".to_vec()) }),
    };
    let result = bounded(run(
        &mut child,
        std::future::ready(Ok(())),
        readers,
        Instant::now() + SAFETY_BOUND,
        SAFETY_BOUND,
        "controlled provider",
    ))
    .await
    .expect("complete direct turn");
    assert_eq!(result.status, "direct child exited");
    assert_eq!(result.stdout, b"stdout");
    assert_eq!(result.stderr, b"stderr");
    assert_eq!(child.waits, 1);
    assert_eq!(child.cleanups, 0);
}

#[tokio::test]
async fn post_exit_pending_readers_time_out_without_child_cleanup_authority() {
    let (readers, releases, drops) = pending_readers().await;
    let error = bounded(collect_after_exit(
        readers,
        Instant::now(),
        Duration::from_millis(250),
        "controlled provider",
    ))
    .await
    .expect_err("post-exit stream deadline");
    let message = error.to_string();
    assert!(message.contains("direct child exited"), "{message}");
    assert!(!message.contains("killed"), "{message}");
    assert!(!message.contains("OS stdin stream"), "{message}");
    both_aborted(drops, releases).await;
}

#[tokio::test]
async fn child_wait_failure_preserves_its_classification_and_aborts_readers() {
    let (readers, releases, drops) = pending_readers().await;
    let mut child = Child::new(Wait::Error);
    let error = bounded(run(
        &mut child,
        std::future::ready(Ok(())),
        readers,
        Instant::now() + SAFETY_BOUND,
        SAFETY_BOUND,
        "controlled provider",
    ))
    .await
    .err()
    .expect("child wait failure");
    assert!(
        error.to_string().contains("failed to run: wait failure"),
        "{error}"
    );
    assert!(!error.to_string().contains("OS stdin stream"), "{error}");
    assert_eq!(child.waits, 1);
    assert_eq!(child.cleanups, 1);
    both_aborted(drops, releases).await;
}

#[tokio::test]
async fn dropping_reader_tasks_aborts_both_readers() {
    let (readers, releases, drops) = pending_readers().await;
    drop(readers);
    both_aborted(drops, releases).await;
}

#[tokio::test]
async fn cancelling_a_pending_session_drops_its_reader_guard() {
    let (readers, releases, drops) = pending_readers().await;
    let (started, writing) = oneshot::channel();
    let mut child = Child::new(Wait::Exit);
    let (writer_dropped, mut observed_writer_drop) = oneshot::channel();
    let writer_resource = DropSignal(Some(writer_dropped));
    let write = async move {
        let _resource = writer_resource;
        let _ = started.send(());
        std::future::pending::<io::Result<()>>().await
    };
    let mut session = Box::pin(run(
        &mut child,
        write,
        readers,
        Instant::now() + SAFETY_BOUND,
        SAFETY_BOUND,
        "controlled provider",
    ));
    bounded(async {
        tokio::select! {
            result = &mut session => panic!("pending writer unexpectedly completed: {}", result.is_ok()),
            ready = writing => ready.expect("writer was polled"),
        }
    }).await;
    drop(session);
    observed_writer_drop
        .try_recv()
        .expect("cancellation dropped writer resource");
    assert_eq!(child.waits, 0);
    assert_eq!(
        child.cleanups, 0,
        "cancellation leaves real child kill-on-drop to parent"
    );
    both_aborted(drops, releases).await;
}
