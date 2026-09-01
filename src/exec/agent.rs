//! The one place a model turn is spawned.
//!
//! A model turn is the only subprocess Anvil runs whose instructions are partly
//! written by whoever opened the pull request: the diff, the review-comment
//! bodies, the conflict text and the PR title all reach a prompt. It is
//! therefore the one subprocess whose environment matters, and it inherited the
//! daemon's whole environment -- `GITHUB_WEBHOOK_SECRET`, `GITHUB_TOKEN`,
//! `SSH_AUTH_SOCK` and every account credential -- at all six sites that spawned
//! one, because a bare `Command::new` inherits by default and nothing said
//! otherwise.
//!
//! [`Posture`] is the isolation decision, and it has no `Default`: a spawn site
//! cannot be written without stating where the turn runs. `Posture::apply`
//! clears the environment and hands back only [`INHERITED`], so a variable
//! reaches a model turn because it is on that list, not because it happened to
//! be set.
//!
//! # What a Posture does not yet carry, and why
//!
//! A per-site tool grant. `agy --help` offers no allowed-tools flag: the whole
//! permission surface is `--dangerously-skip-permissions`, `--mode`
//! (`accept-edits` | `plan`) and `--sandbox`. Its auto-deny message names the
//! real mechanism -- an allow-rule under `permissions.allow` in a
//! `settings.json`, spelled `command(<target>)` -- but the file's discovery
//! path could not be established here: rules placed under
//! `ANTIGRAVITY_CONFIG_DIR`, `GEMINI_CLI_CONFIG_DIR`, `.agy/`, `.antigravity/`
//! and the working directory were each ignored, and `--mode plan` alone still
//! auto-denies.
//!
//! So the field is absent rather than guessed. A `Posture` that wrote a
//! settings.json nothing reads would look like isolation while granting
//! nothing, and every probe would fail with "permission check failed for
//! command" -- which `doc_guard`'s own comment records happening once already.
//! Grants land when the discovery path is known.

use super::inherited::INHERITED;
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// A provider CLI command that cannot be handed to a raw subprocess runner.
/// It can leave this wrapper only through an `exec` transport that also
/// requires a `ModelPrompt`.
///
/// ```compile_fail
/// let cmd = anvil::exec::agy_agent(
///     &anvil::exec::Posture::in_workspace(std::path::Path::new(".")),
///     "high",
///     std::time::Duration::from_secs(600),
///     None,
/// );
/// let _ = anvil::exec::run_bounded(
///     cmd.unwrap(),
///     anvil::exec::ExecClass::Model,
///     "unchecked model turn",
/// );
/// ```
///
/// Provider argv is sealed too. Contributor text cannot be smuggled into a
/// provider option after construction:
///
/// ```compile_fail
/// let review_body = "--model\nattacker-selected";
/// let mut cmd = anvil::exec::claude_agent(
///     &anvil::exec::Posture::in_workspace(std::path::Path::new(".")),
///     "claude-3-7-sonnet",
/// ).unwrap();
/// cmd.arg(review_body);
/// ```
///
/// The raw STDIN runner is not an alternate public model transport:
///
/// ```compile_fail
/// let cmd = tokio::process::Command::new("agy");
/// let _ = anvil::exec::run_bounded_with_stdin(
///     cmd,
///     "unchecked review body",
///     std::time::Duration::from_secs(30),
///     "raw model turn",
/// );
/// ```
pub struct AgentCommand {
    command: Command,
    framing: Framing,
}

/// A provider presence probe with a complete, finite argv. Unlike a model
/// turn, it carries no prompt and never enters a raw non-model runner.
struct ProviderProbeCommand(Command);

/// How the selected provider accepts a prompt on STDIN.
///
/// This is deliberately finite and private. A provider cannot choose an
/// arbitrary formatter at a call site, and the formatter never receives raw
/// contributor text except inside the sealed transport below.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Framing {
    Plain,
    AgyStreamJson,
}

impl AgentCommand {
    fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        self.command.args(args);
        self
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn as_std(&self) -> &std::process::Command {
        self.command.as_std()
    }
}

mod provider;
mod transport;
pub(super) use provider::is_provider_program;
pub use provider::{agy_agent, claude_agent, codex_agent, cursor_agent, grok_agent};
pub(crate) use transport::ModelPromptPermit;

/// Typed facade over the private transport. No raw command, prompt bytes,
/// formatter, or permit crosses this module boundary.
pub(super) async fn deliver(
    command: AgentCommand,
    prompt: &crate::model_prompt::ModelPrompt,
    limit: std::time::Duration,
    what: &str,
) -> anyhow::Result<std::process::Output> {
    transport::deliver(command, prompt, limit, what).await
}

/// Checks the one provider readiness condition used by the CLI environment
/// report. No raw `Command` or provider argv leaves the finite provider seam.
pub(crate) async fn probe_agy_help() -> anyhow::Result<std::process::Output> {
    transport::probe(
        provider::agy_help_probe(),
        crate::exec::ExecClass::Quick.timeout(),
        "agy --help",
    )
    .await
}

/// Where a model turn runs, and what it carries.
///
/// No `Default`, and no constructor that omits the workspace: a new spawn site
/// must say which tree the turn is pointed at. The daemon's own working
/// directory is what a turn runs in when nobody chooses, and that is Anvil's
/// checkout rather than the repository under review.
#[derive(Debug, Clone)]
pub struct Posture {
    workspace: PathBuf,
    credentials: Vec<(String, String)>,
}

impl Posture {
    /// A turn that runs in `workspace` and inherits only [`INHERITED`].
    pub fn in_workspace(workspace: impl Into<PathBuf>) -> Self {
        Self {
            workspace: workspace.into(),
            credentials: Vec::new(),
        }
    }

    /// Adds one variable beyond [`INHERITED`], for a credential the caller
    /// holds rather than one the daemon's environment happens to carry.
    ///
    /// This is how the account pool hands a leased token to the turn it leased
    /// it for, without that token being present for every other turn.
    #[must_use]
    pub fn with_credential(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.credentials.push((name.into(), value.into()));
        self
    }

    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Clears the environment, restores [`INHERITED`] and this posture's
    /// credentials, and sets the working directory.
    ///
    /// Order matters: `env_clear` first, then the allowlist, so a variable is
    /// present because it was named and not because it survived.
    pub fn apply(&self, cmd: &mut Command) {
        self.apply_from(cmd, std::env::vars());
    }

    /// [`Posture::apply`], against a stated environment rather than this
    /// process's own.
    ///
    /// The filter is the security property, so it must be exercisable against
    /// an environment that actually holds a secret. Mutating this process's
    /// environment to arrange that is a data race the standard library marks
    /// `unsafe`, and a check that can only be run where the thing it forbids is
    /// absent is not a check.
    pub fn apply_from<I>(&self, cmd: &mut Command, environment: I)
    where
        I: IntoIterator<Item = (String, String)>,
    {
        cmd.env_clear();
        for (name, value) in environment {
            if INHERITED.contains(&name.as_str()) {
                cmd.env(name, value);
            }
        }
        for (name, value) in &self.credentials {
            cmd.env(name, value);
        }
        // Point `gh` at a directory that holds no host configuration, so a
        // `gh` invoked from inside a turn is not already logged in as Anvil.
        //
        // A narrowing of the tool path, NOT a containment boundary, and the
        // difference matters: `HOME` is on `INHERITED` because a provider CLI
        // cannot start without it, and the forge token lives under it, so a
        // turn that reads the file directly still reads it. This closes the
        // path where the turn does not have to try -- `gh` finding the
        // credential by itself -- and `inherited.rs` states the residual.
        cmd.env("GH_CONFIG_DIR", self.workspace.join(".anvil-no-gh-config"));
        cmd.current_dir(&self.workspace);
    }
}

/// Applies the model-turn posture to a command chosen by the finite provider
/// seam in the private `exec::agent::provider` child module.
fn command(tool: &str, posture: &Posture, framing: Framing) -> AgentCommand {
    command_in(tool, posture, framing, std::env::vars())
}

/// [`command`], against a stated environment. Used by posture unit tests.
fn command_in<I>(tool: &str, posture: &Posture, framing: Framing, environment: I) -> AgentCommand
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut cmd = Command::new(tool);
    posture.apply_from(&mut cmd, environment);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    AgentCommand {
        command: cmd,
        framing,
    }
}

#[cfg(test)]
mod tests;
