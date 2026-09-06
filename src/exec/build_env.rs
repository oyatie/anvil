//! What a build or a test run is allowed to carry in from the daemon.
//!
//! Its own list, and not [`super::inherited::INHERITED`], because the two
//! subjects differ: that one bounds a model turn talking to a provider CLI,
//! this one bounds `cargo` and `npm` compiling and running a CONTRIBUTOR'S
//! code.
//!
//! # Why this exists
//!
//! `run_cargo_test_gate` executes every `#[test]` in the pull request's branch.
//! Until this list, it did so in the daemon's own environment, which holds
//! `GITHUB_WEBHOOK_SECRET` (`config.rs::from_env`) — the secret that
//! authenticates deliveries to this daemon. Anyone who can open a pull request
//! can add a test, and a test can read an environment variable.
//!
//! A type-check never ran that code. Running the suite is a real improvement to
//! the gate's fidelity, and it is what introduced the exposure.
//!
//! # What this list does NOT bound
//!
//! The same caveat `INHERITED` carries. `HOME` is here because cargo and rustup
//! cannot find their configuration without it, so a test that reads files still
//! reads whatever that user can read. This stops the daemon HANDING OVER its
//! secrets; it is not a sandbox, and calling it one would be the fabrication
//! this repository exists to refuse.

/// Environment variables a build or test run is given.
///
/// A toolchain needs to be found, to locate its own configuration, and to write
/// temporary files. It does not need the daemon's forge credentials, its
/// webhook secret, or its model provider keys.
pub const BUILD_INHERITED: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "TZ",
    // Toolchain roots. Without these a rustup-managed cargo cannot resolve a
    // toolchain, and the gate reports a build failure that is the daemon's
    // fault rather than the pull request's.
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTUP_TOOLCHAIN",
    // macOS toolchain discovery; absent on Linux runners and harmless there.
    "DEVELOPER_DIR",
    "SDKROOT",
];

/// Names that must never reach a build, whatever else changes.
///
/// Asserted rather than assumed: a prefix is a rule, and the list above is a
/// list. If someone adds `GITHUB_TOKEN` to `BUILD_INHERITED` by hand, the test
/// beside this refuses it.
pub const NEVER_HANDED_OVER: &[&str] = &[
    "GITHUB_WEBHOOK_SECRET",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
];

/// A command that carries only what a toolchain needs.
///
/// A constructor rather than a scrub applied afterwards, for the reason
/// `ARCHITECTURE.md` gives for `exec::gh()` and the finite provider constructors: a call site
/// that has to remember to scrub is a call site that can forget, and the next
/// build spawned here would inherit the daemon's environment in full.
pub fn command(program: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    apply(&mut cmd);
    cmd
}

/// Clear the environment and hand back only what a toolchain needs.
pub fn apply(cmd: &mut tokio::process::Command) {
    super::non_model::clear_environment(cmd);
    for name in BUILD_INHERITED {
        if let Ok(value) = std::env::var(name) {
            cmd.env(name, value);
        }
    }
}
