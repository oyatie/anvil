//! The webhook transport must survive its own upstream dropping.
//!
//! `gh webhook forward` holds a WebSocket to webhook-forwarder.github.com and
//! loses it routinely:
//!
//!   Error: error receiving json event: websocket: close 1006 (abnormal
//!   closure): unexpected EOF
//!
//! The spawn site checked only `if let Err(e) = cmd.status().await`. `status()`
//! returns `Ok(status)` when the child runs and exits -- `Err` fires only when
//! the child cannot be *spawned*. So a forwarder killed by a 1006 drop returned
//! `Ok(non-zero)`, logged nothing, and was never restarted. Webhook delivery for
//! that repository stopped silently until the daemon was restarted.
//!
//! Observed: 1h38m of uptime whose only output was telemetry polling, and a
//! forwarder count that fell from 3 to 2 within five minutes of a fresh start.
//!
//! These tests pin the supervision behaviour, not the transport.

use anvil::webhook::forwarder_supervisor::{RestartPolicy, next_restart_delay, supervise_bounded};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

fn test_policy() -> RestartPolicy {
    RestartPolicy {
        base_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(4),
    }
}

#[tokio::test]
async fn a_forwarder_that_exits_is_restarted() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let a = attempts.clone();

    // A child that always exits immediately, as a 1006-dropped forwarder does.
    supervise_bounded("test-repo", &test_policy(), 3, move || {
        let a = a.clone();
        async move {
            a.fetch_add(1, Ordering::SeqCst);
            Ok(1i32) // non-zero exit: ran, then died
        }
    })
    .await;

    assert_eq!(
        attempts.load(Ordering::SeqCst),
        3,
        "a forwarder that exits must be restarted; without this a single WebSocket \
         drop silently ends webhook delivery for that repository"
    );
}

#[tokio::test]
async fn a_clean_exit_is_still_restarted() {
    // gh may exit 0 on a dropped socket. Exit code must not decide whether the
    // transport should still be running -- it must be running either way.
    let attempts = Arc::new(AtomicUsize::new(0));
    let a = attempts.clone();

    supervise_bounded("test-repo", &test_policy(), 2, move || {
        let a = a.clone();
        async move {
            a.fetch_add(1, Ordering::SeqCst);
            Ok(0i32)
        }
    })
    .await;

    assert_eq!(
        attempts.load(Ordering::SeqCst),
        2,
        "exit 0 is still an outage"
    );
}

#[test]
fn restart_delay_grows_with_consecutive_failures() {
    let p = RestartPolicy {
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(60),
    };
    let first = next_restart_delay(1, &p);
    let later = next_restart_delay(5, &p);
    assert!(
        later > first,
        "delay must grow: a forwarder failing because GitHub is degraded should not \
         be respawned in a tight loop ({:?} -> {:?})",
        first,
        later
    );
}

#[test]
fn restart_delay_is_capped() {
    let p = RestartPolicy {
        base_delay: Duration::from_millis(100),
        max_delay: Duration::from_secs(30),
    };
    for attempt in 1..=40 {
        assert!(
            next_restart_delay(attempt, &p) <= p.max_delay,
            "attempt {} exceeded the cap; unbounded backoff means a transport that \
             never comes back",
            attempt
        );
    }
}

#[test]
fn restart_delay_is_jittered() {
    let p = RestartPolicy {
        base_delay: Duration::from_millis(500),
        max_delay: Duration::from_secs(60),
    };
    // Three forwarders failing together must not retry in lockstep.
    let samples: Vec<Duration> = (0..12).map(|_| next_restart_delay(4, &p)).collect();
    let distinct = samples
        .iter()
        .map(|d| d.as_millis())
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert!(
        distinct > 1,
        "identical delays every time means no jitter, so all three forwarders \
         reconnect in a thundering herd: {:?}",
        samples
    );
}

#[test]
fn the_spawn_site_does_not_treat_a_running_child_that_died_as_success() {
    let src = std::fs::read_to_string("src/cli/server.rs").expect("server.rs");
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !code.contains("if let Err(e) = cmd.status().await"),
        "`status()` yields Ok(status) when the child ran and exited; only a spawn \
         failure is Err. Checking only Err makes every real forwarder death silent."
    );
}
