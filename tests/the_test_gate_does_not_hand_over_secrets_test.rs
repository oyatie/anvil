//! A build spawned by the daemon carries no daemon secret.
//!
//! `run_cargo_test_gate` executes every `#[test]` in the pull request's branch.
//! Until `exec::build_env` it did so in the daemon's own environment, which
//! holds `GITHUB_WEBHOOK_SECRET` — the secret that authenticates deliveries to
//! this daemon. Anyone who can open a pull request can add a test, and a test
//! can read an environment variable.
//!
//! Measured by running a real child and reading what it saw, not by asserting
//! that a scrubbing function is called. Four guards in this tree have been
//! satisfied by the presence of a call while the property did not hold.

use anvil::exec::build_env::{BUILD_INHERITED, NEVER_HANDED_OVER};

/// The list is a list; this is the rule it must obey.
#[test]
fn no_secret_name_is_on_the_allowlist() {
    for forbidden in NEVER_HANDED_OVER {
        assert!(
            !BUILD_INHERITED.contains(forbidden),
            "`{forbidden}` is on the build allowlist. A build runs contributor \
             code; handing it the daemon's credentials makes every pull request \
             an exfiltration path."
        );
    }
}

/// And the scrubbing works on a real process.
#[cfg(unix)]
#[tokio::test]
async fn a_spawned_build_cannot_read_the_daemons_webhook_secret() {
    // SAFETY: single-threaded test process, set before any child is spawned.
    unsafe {
        std::env::set_var("GITHUB_WEBHOOK_SECRET", "the-daemons-actual-secret");
        std::env::set_var("ANTHROPIC_API_KEY", "the-daemons-actual-key");
    }

    let mut cmd = tokio::process::Command::new("/usr/bin/env");
    anvil::exec::build_env::apply(&mut cmd);
    let out = cmd.output().await.expect("env runs");
    let seen = String::from_utf8_lossy(&out.stdout);

    for forbidden in NEVER_HANDED_OVER {
        assert!(
            !seen
                .lines()
                .any(|l| l.starts_with(&format!("{forbidden}="))),
            "a build spawned by the daemon was handed `{forbidden}`. Every \
             `#[test]` in a contributor's branch runs in that environment."
        );
    }
    assert!(
        !seen.contains("the-daemons-actual-secret"),
        "the webhook secret reached the child under some other name"
    );

    // The other half: a toolchain still gets what it needs. A scrub that
    // starves the build reports a failure that is the daemon's fault and
    // publishes it as the pull request's.
    assert!(
        seen.lines().any(|l| l.starts_with("PATH=")),
        "PATH did not survive the scrub, so no toolchain can be found"
    );

    unsafe {
        std::env::remove_var("GITHUB_WEBHOOK_SECRET");
        std::env::remove_var("ANTHROPIC_API_KEY");
    }
}
