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

/// Builds the spawn for a model turn.
///
/// Every model turn in the tree is constructed here, so the environment
/// decision cannot be forgotten at a seventh site -- and
/// `tests/model_spawns_go_through_one_seam_test.rs` refuses one that is.
pub fn agent(tool: &str, posture: &Posture) -> Command {
    agent_in(tool, posture, std::env::vars())
}

/// [`agent`], against a stated environment. See [`Posture::apply_from`].
pub fn agent_in<I>(tool: &str, posture: &Posture, environment: I) -> Command
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut cmd = Command::new(tool);
    posture.apply_from(&mut cmd, environment);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A daemon environment, as the webhook process actually carries it.
    fn daemon_environment() -> Vec<(String, String)> {
        vec![
            (
                "PATH".to_string(),
                std::env::var("PATH").unwrap_or_default(),
            ),
            (
                "GITHUB_WEBHOOK_SECRET".to_string(),
                "anvil-test-secret-2f9c".to_string(),
            ),
            (
                "GITHUB_TOKEN".to_string(),
                "ghp_anvil_test_token".to_string(),
            ),
            (
                "SSH_AUTH_SOCK".to_string(),
                "/tmp/anvil-test-agent.sock".to_string(),
            ),
        ]
    }

    /// The whole point, exercised end to end against an environment that holds
    /// the secrets: none of them reaches the child.
    #[tokio::test]
    async fn a_daemon_secret_does_not_reach_a_model_turn() {
        let posture = Posture::in_workspace(std::env::temp_dir());
        let cmd = agent_in("env", &posture, daemon_environment());
        let out = crate::exec::run_bounded(cmd, crate::exec::ExecClass::Quick, "env")
            .await
            .expect("env runs");
        let seen = String::from_utf8_lossy(&out.stdout).to_string();

        // Names, never values. A failure message that prints the child's
        // environment to prove a secret was in it has put the secret in the
        // build log, which is the defect in a second place.
        assert!(
            seen.contains("GH_CONFIG_DIR="),
            "a `gh` run from inside a turn must not find Anvil's own host \
             configuration"
        );

        let leaked: Vec<&str> = seen
            .lines()
            .filter(|line| {
                [
                    "anvil-test-secret-2f9c",
                    "ghp_anvil_test_token",
                    "agent.sock",
                ]
                .iter()
                .any(|s| line.contains(s))
            })
            .filter_map(|line| line.split('=').next())
            .collect();
        assert!(
            leaked.is_empty(),
            "these variables reached a model turn: {leaked:?}"
        );

        // The env is a SUBSET of what was named, which is the assertion that
        // exercises `env_clear`. Checking only that three fixture literals were
        // filtered tests the allowlist and nothing else: those literals arrive
        // through the synthetic environment, the allowlist alone keeps them
        // out, and `env_clear` touches only the REAL parent environment, which
        // does not contain them. Deleting `env_clear` left this whole module
        // green -- measured, by deleting it.
        let allowed: std::collections::BTreeSet<&str> =
            INHERITED.iter().copied().chain(["GH_CONFIG_DIR"]).collect();
        let unexpected: Vec<&str> = seen
            .lines()
            .filter_map(|l| l.split('=').next())
            .filter(|n| !n.is_empty() && !allowed.contains(n))
            .collect();
        assert!(
            unexpected.is_empty(),
            "the turn was handed variables nobody named: {unexpected:?}. The \
             daemon's environment reaches a model turn unless it is cleared \
             first, so this is what `env_clear` is for."
        );

        // And the turn is still able to run: PATH is on the list, so the tool
        // can find its own helpers. A posture that starves the turn is not
        // isolation, it is breakage.
        let names: Vec<&str> = seen.lines().filter_map(|l| l.split('=').next()).collect();
        assert!(
            names.contains(&"PATH"),
            "PATH must survive; the turn was given {names:?}"
        );
    }

    #[tokio::test]
    async fn a_leased_credential_reaches_the_turn_it_was_leased_for() {
        let posture = Posture::in_workspace(std::env::temp_dir())
            .with_credential("GEMINI_API_KEY", "leased-for-this-turn");
        let cmd = agent("env", &posture);
        let out = crate::exec::run_bounded(cmd, crate::exec::ExecClass::Quick, "env")
            .await
            .expect("env runs");
        assert!(
            String::from_utf8_lossy(&out.stdout).contains("leased-for-this-turn"),
            "a credential the caller supplied must reach the turn"
        );
    }

    #[tokio::test]
    async fn the_turn_runs_in_the_workspace_it_was_given() {
        let dir = std::env::temp_dir().join("anvil-posture-cwd");
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let cmd = agent("pwd", &Posture::in_workspace(&dir));
        let out = crate::exec::run_bounded(cmd, crate::exec::ExecClass::Quick, "pwd")
            .await
            .expect("pwd runs");
        let seen = String::from_utf8_lossy(&out.stdout);
        assert!(
            seen.trim().ends_with("anvil-posture-cwd"),
            "the turn ran in {seen:?}, not in the workspace it was given"
        );
    }
}
