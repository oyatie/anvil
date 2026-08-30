//! Unit tests for the bounded executor: same crate, so private items stay
//! reachable.
//!
//! A file of its own to keep `mod.rs` inside ADR-0719 D-35's 300-line
//! budget, which does not exempt tests.

#[test]
fn agy_print_timeout_sits_a_margin_under_anvils_bound() {
    use super::{ExecClass, agy_print_timeout_arg};
    use std::time::Duration;
    assert_eq!(
        agy_print_timeout_arg(ExecClass::Model.timeout()),
        "570s",
        "600s Model bound minus the 30s margin"
    );
    assert_eq!(agy_print_timeout_arg(Duration::from_secs(420)), "390s");
    // Never 0s: agy reads that as "do not wait" and the turn dies at once.
    assert_eq!(agy_print_timeout_arg(Duration::from_secs(5)), "1s");
    assert_eq!(agy_print_timeout_arg(Duration::ZERO), "1s");
}

use super::*;

#[tokio::test]
async fn returns_output_for_a_fast_command() {
    let mut c = Command::new("echo");
    c.arg("hello");
    let out = run_bounded(c, ExecClass::Quick, "echo").await.expect("ok");
    assert!(String::from_utf8_lossy(&out.stdout).contains("hello"));
}

#[tokio::test]
async fn a_hung_command_is_killed_and_reported_as_an_error() {
    let mut c = Command::new("sleep");
    c.arg("30");
    let err = run_bounded_for(c, Duration::from_millis(200), "sleep")
        .await
        .expect_err("must time out");
    let msg = err.to_string();
    assert!(msg.contains("timed out"), "unexpected: {msg}");
}

#[tokio::test]
async fn a_missing_binary_is_an_error_not_a_silent_pass() {
    let c = Command::new("anvil-no-such-binary-xyz");
    let err = run_bounded(c, ExecClass::Quick, "probe")
        .await
        .expect_err("must error");
    assert!(err.to_string().contains("failed to run"));
}

#[test]
fn timeouts_are_ordered_by_expected_cost() {
    assert!(ExecClass::Quick.timeout() < ExecClass::Api.timeout());
    assert!(ExecClass::Api.timeout() < ExecClass::Vcs.timeout());
    assert!(ExecClass::Vcs.timeout() < ExecClass::Model.timeout());
    assert!(ExecClass::Model.timeout() < ExecClass::Build.timeout());
}
