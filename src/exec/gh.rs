//! The one place a `gh` command is built.
//!
//! `ARCHITECTURE.md` M6 specifies this seam as `exec::gh() -> Command`, with
//! the note "installation token, not ambient". This is the constructor half.
//! Until the token half lands, `gh` resolves its own credential exactly as it
//! did before -- what changes here is that there is now a single place for the
//! token to be injected, instead of thirty-four.
//!
//! # Why a seam, before any token exists
//!
//! Thirty-four `Command::new("gh")` sites across ten modules each inherited the
//! daemon's whole environment: `GITHUB_WEBHOOK_SECRET`, every model provider
//! key, `SSH_AUTH_SOCK`. `gh` needs none of them. That was not thirty-four
//! oversights, it was the default, and the thirty-fifth site would have been
//! written the same way.
//!
//! # Why this is not [`super::build_env`]
//!
//! The subjects are opposites. A build runs a CONTRIBUTOR'S code, so it must
//! never see a forge credential -- `build_env::NEVER_HANDED_OVER` names
//! `GH_TOKEN` explicitly. `gh` is Anvil talking to the forge as itself, so this
//! is the one seam where a forge credential is exactly what belongs. Reusing
//! that list here would flip a `GH_TOKEN`-authenticated deployment to
//! keyring-or-nothing without saying so.

use tokio::process::Command;

/// What a `gh` invocation is given.
///
/// `HOME` is not optional: `gh`'s own credential store lives under it, so a
/// keyring-authenticated deployment loses its authentication without it. That
/// also means this is not a sandbox -- `gh` reads whatever that user can read.
/// It bounds what the daemon HANDS OVER, which is a smaller claim and the only
/// one the list supports.
pub const GH_INHERITED: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "LOGNAME",
    "TMPDIR",
    "LANG",
    "LC_ALL",
    "TZ",
    // The forge credential, when the deployment supplies one this way rather
    // than through `gh`'s keyring. Allowed HERE and at no other seam.
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GH_HOST",
    // Where `gh` looks for the credential it stored. `GH_CONFIG_DIR` wins when
    // set; otherwise `gh` resolves `$XDG_CONFIG_HOME/gh` before `$HOME/.config`,
    // so clearing the XDG roots relocates the config of any deployment that
    // sets them and unauthenticates the daemon with no error to read.
    "GH_CONFIG_DIR",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_STATE_HOME",
    // A corporate TLS interception root. Go reads these, and without them the
    // failure surfaces as a certificate error rather than a configuration one.
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    // Corporate egress. Without these `gh` cannot reach the forge at all, and
    // the failure reads as an API error rather than a proxy one.
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
];

/// Names that must never reach a `gh` invocation, whatever else changes.
///
/// Asserted rather than assumed, for the reason `build_env` gives: the list
/// above is a list, and a name appended to it by hand is refused by the test
/// beside this one.
pub const NEVER_HANDED_OVER: &[&str] = &[
    "GITHUB_WEBHOOK_SECRET",
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "GEMINI_API_KEY",
    "AWS_SECRET_ACCESS_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GITHUB_APP_PRIVATE_KEY",
];

/// A `gh` command carrying only what `gh` needs.
///
/// A constructor rather than a scrub applied afterwards: a call site that
/// forgets to scrub compiles, and a call site that cannot reach `Command::new`
/// does not. Returns the `Command` rather than running it, because
/// `gh webhook forward` is a supervised long-running process and not every
/// caller goes through `run_bounded`.
pub fn command() -> Command {
    let mut cmd = Command::new("gh");
    apply(&mut cmd);
    cmd
}

/// Applies the environment bound to a command the caller already holds.
pub fn apply(cmd: &mut Command) {
    cmd.env_clear();
    for name in GH_INHERITED {
        if let Ok(value) = std::env::var(name) {
            cmd.env(name, value);
        }
    }
}
