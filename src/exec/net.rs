//! The one place an outbound network tool is spawned.
//!
//! The fourth outbound seam, alongside `exec::agent`, [`super::build_env`]
//! and `exec::gh`. Those three bound a model turn, a build and a forge call;
//! this one bounds a transport -- `curl` reaching a public HTTP endpoint.
//!
//! # Why a seam
//!
//! `supply_chain_guard::osv_stream` POSTs to the OSV advisory database once per
//! certification. A bare `Command::new` hands that child the daemon's whole
//! environment: `GITHUB_WEBHOOK_SECRET`, `GH_TOKEN` and every model provider
//! key. That is the default rather than an oversight, so the next network call
//! written in that module carries them too.
//!
//! # Severity, stated honestly
//!
//! Lower than [`super::build_env`]. That seam exists because
//! `run_cargo_test_gate` runs a CONTRIBUTOR'S `#[test]` code, and a test can
//! read an environment variable -- so anyone who could open a pull request
//! could read the webhook secret. Nothing comparable holds here: the argv is
//! fixed by the caller and `curl` cannot be made to print its environment by a
//! payload. This is a consistency and blast-radius fix. Calling it an exposure
//! would be the overclaim this repository exists to refuse.
//!
//! # Why this is not `exec::gh`
//!
//! `gh` is Anvil talking to the forge as itself, so a forge credential is
//! exactly what belongs at that seam. A transport to a public advisory
//! database authenticates nobody, so `GH_TOKEN` is on [`NEVER_HANDED_OVER`]
//! here. The two lists disagree on purpose, and the test beside this pins the
//! disagreement in both directions.

use tokio::process::Command;

/// What an outbound network tool is given.
///
/// Shorter than the other three lists because the subject is smaller: resolve a
/// name, open a TLS connection, write a body, read the answer. As with every
/// other seam this bounds what the daemon HANDS OVER; it is not a sandbox, and
/// that smaller claim is the only one the list supports.
pub const NET_INHERITED: &[&str] = &[
    // Without this the tool is not found at all and every request fails as a
    // spawn error rather than as the network result it never got to make.
    "PATH",
    // `curl` reads `~/.curlrc` on every invocation, which is where a
    // deployment sets its proxy, its CA path, and whether `~/.netrc` applies.
    // Clearing `HOME` discards that configuration with nothing said.
    "HOME",
    // Where a transport spools a body too large to hold in memory.
    "TMPDIR",
    // Corporate egress. Without these the request never leaves the box, and the
    // failure reads as an outage at the far end rather than a proxy here.
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    // Both spellings, because libcurl honours `http_proxy` in lowercase ONLY --
    // it ignores the uppercase form so a CGI `Proxy:` header cannot set it.
    "https_proxy",
    "http_proxy",
    "no_proxy",
    // A corporate TLS interception root. curl and Go both read these, and
    // without them the failure surfaces as a certificate error rather than the
    // configuration error it is.
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
];

/// Names that must never reach an outbound network tool, whatever else changes.
///
/// Asserted rather than assumed, for the reason `build_env` gives: the list
/// above is a list, and a name appended to it by hand is refused by the test
/// beside this one. `GH_TOKEN` and `GITHUB_TOKEN` are here and NOT on
/// [`super::gh::GH_INHERITED`]'s exclusions -- a forge credential belongs at
/// exactly one seam, and this is not it.
pub const NEVER_HANDED_OVER: &[&str] = &[
    "GITHUB_WEBHOOK_SECRET",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GITHUB_APP_PRIVATE_KEY",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
];

/// A network tool carrying only what a transport needs.
///
/// A constructor rather than a scrub applied afterwards, for the reason the
/// other three seams give: a call site that forgets to scrub compiles, and a
/// call site that cannot reach `Command::new` does not. Returns the `Command`
/// rather than running it, so the caller keeps its own budget and its own
/// choice of runner.
///
/// `program` is a parameter because the caller takes one, so every way the
/// subprocess can fail stays reachable from a test that makes no network
/// request. Production passes `curl`.
pub fn command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    apply(&mut cmd);
    cmd
}

/// Applies the environment bound to a command the caller already holds.
pub fn apply(cmd: &mut Command) {
    cmd.env_clear();
    for name in NET_INHERITED {
        if let Ok(value) = std::env::var(name) {
            cmd.env(name, value);
        }
    }
}
