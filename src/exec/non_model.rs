//! Checked transport for subprocesses that are not model providers.
//!
//! Raw `Command` remains useful to the repository's many git/build/forge
//! call sites, but it is not itself an execution capability. Every raw runner
//! converts it to the private [`NonModelCommand`] below. Conversion accepts a
//! finite tool vocabulary, rejects shell/environment launchers, and resolves
//! aliases before execution so a symlink named `git` cannot really be `agy`.
//!
//! This is a direct-executable capability boundary, not process containment.
//! Several admitted tools (`cargo`, `git`, `node`, `npm`, and `python3`) can
//! themselves launch descendants, and contributor builds intentionally run
//! contributor code. Preventing an admitted tool or hostile build from
//! starting an arbitrary descendant requires a sandbox/process policy outside
//! this prompt-transport seal. The property here is narrower: safe in-tree
//! code cannot hand a provider command or raw prompt directly to these runners
//! by spelling a provider, `env`, a shell, or a provider symlink.

use anyhow::{Result, bail};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::time::Duration;
use tokio::process::Command;

use super::{ExecClass, agent};

const NON_MODEL_PROGRAMS: &[&str] = &[
    "cargo", "cedar", "echo", "gh", "git", "go", "node", "npm", "ps", "python3", "sleep",
];
const CANONICAL_PROGRAM_ALIASES: &[(&str, &str)] = &[
    // These launchers require Unix `arg0` restoration to retain their
    // requested multicall semantics after binding the canonical executable.
    #[cfg(unix)]
    ("cargo", "rustup"),
    #[cfg(unix)]
    ("npm", "npm-cli.js"),
    // Rust's Windows process launcher supports command shims, while rebinding
    // a cargo/rustup or npm/JS alias cannot preserve its requested semantics.
    #[cfg(windows)]
    ("npm", "npm.cmd"),
];
const CLEARED_ENV_MARKER: &str = "ANVIL_INTERNAL_NON_MODEL_ENV_CLEARED";

/// A raw command that has passed the finite non-model admission policy.
/// Its inner command never leaves this module.
struct NonModelCommand(Command);

/// Synchronous counterpart for call chains and destructors that cannot await.
struct SyncNonModelCommand(std::process::Command);

struct ResolvedExecutable {
    canonical: PathBuf,
    requested_name: String,
}

impl NonModelCommand {
    fn checked(command: Command) -> Result<Self> {
        Self::checked_for(command, NON_MODEL_PROGRAMS)
    }

    fn checked_for(command: Command, admitted: &[&str]) -> Result<Self> {
        let resolved = validate_program_for(&command, admitted)?;
        Ok(Self(Command::from(bind_std_program(
            command.as_std(),
            &resolved.canonical,
            &resolved.requested_name,
        ))))
    }
}

impl SyncNonModelCommand {
    fn checked(command: std::process::Command) -> Result<Self> {
        let resolved = validate_std_program(&command)?;
        Ok(Self(bind_std_program(
            &command,
            &resolved.canonical,
            &resolved.requested_name,
        )))
    }
}

/// Clears a command environment in a way the canonical-program rebinder can
/// preserve. The marker is consumed before execution and never reaches the
/// child.
pub(super) fn clear_environment(command: &mut Command) {
    command.env_clear();
    command.env(CLEARED_ENV_MARKER, "1");
}

#[path = "net.rs"]
mod net;
pub(in crate::exec) mod resolution;
mod transport;

use resolution::resolve_executable;

pub(super) async fn run(command: Command, class: ExecClass, what: &str) -> Result<Output> {
    let command = NonModelCommand::checked(command)?;
    transport::run_for(command, class.timeout(), what, Some(class)).await
}

pub(super) async fn run_for(command: Command, limit: Duration, what: &str) -> Result<Output> {
    let command = NonModelCommand::checked(command)?;
    transport::run_for(command, limit, what, None).await
}

/// Executes the one finite non-forge HTTP request. No raw curl command,
/// destination, or payload crosses this module boundary.
pub(super) async fn post_osv_batch(
    packages: &[crate::supply_chain_guard::LockedPackage],
) -> Result<Output> {
    net::post_osv_batch(packages).await
}

pub(super) async fn run_with_stdin(
    command: Command,
    stdin_payload: &str,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    let command = NonModelCommand::checked(command)?;
    transport::run_with_stdin(command, stdin_payload, limit, what).await
}

pub(super) async fn run_status(command: Command, what: &str) -> Result<ExitStatus> {
    let command = NonModelCommand::checked(command)?;
    transport::run_status(command, what).await
}

pub(super) fn run_sync_bounded(
    command: std::process::Command,
    limit: Duration,
    what: &str,
) -> Result<Output> {
    let command = SyncNonModelCommand::checked(command)?;
    transport::run_sync_bounded(command, limit, what)
}

fn validate_program_for(command: &Command, admitted: &[&str]) -> Result<ResolvedExecutable> {
    validate_std_program_for(command.as_std(), admitted)
}

fn validate_std_program(command: &std::process::Command) -> Result<ResolvedExecutable> {
    validate_std_program_for(command, NON_MODEL_PROGRAMS)
}

fn validate_std_program_for(
    command: &std::process::Command,
    admitted: &[&str],
) -> Result<ResolvedExecutable> {
    let requested = command.get_program();
    let requested_name = executable_name(requested)
        .ok_or_else(|| anyhow::anyhow!("raw subprocess program has no valid executable name"))?;

    if agent::is_provider_program(requested) {
        bail!("model provider commands require the typed AgentCommand + ModelPrompt transport");
    }

    if !admitted.contains(&requested_name) {
        bail!(
            "raw subprocess program {requested_name:?} is outside the finite non-model tool seam"
        );
    }

    let resolved = resolve_executable(command).ok_or_else(|| {
        anyhow::anyhow!(
            "approved raw subprocess {requested_name:?} could not be resolved to a runnable regular file"
        )
    })?;
    if agent::is_provider_program(resolved.canonical.as_os_str()) {
        bail!(
            "raw subprocess alias resolves to a model provider; use the typed AgentCommand + ModelPrompt transport"
        );
    }
    let canonical_name = executable_name(resolved.canonical.as_os_str())
        .ok_or_else(|| anyhow::anyhow!("resolved raw subprocess has no valid executable name"))?;
    if !canonical_name_is_admitted(requested_name, canonical_name) {
        bail!(
            "raw subprocess alias {requested_name:?} resolves outside the finite non-model tool seam as {canonical_name:?}"
        );
    }
    let trusted = resolve_executable(&std::process::Command::new(requested_name)).ok_or_else(|| {
        anyhow::anyhow!(
            "approved raw subprocess {requested_name:?} has no trusted executable on the service PATH"
        )
    })?;
    if resolved.canonical != trusted.canonical {
        bail!(
            "raw subprocess {requested_name:?} resolves to {}, not the trusted installed executable {}",
            resolved.canonical.display(),
            trusted.canonical.display()
        );
    }
    Ok(ResolvedExecutable {
        requested_name: requested_name.to_owned(),
        ..resolved
    })
}

fn canonical_name_is_admitted(requested: &str, canonical: &str) -> bool {
    if requested == canonical {
        return true;
    }
    CANONICAL_PROGRAM_ALIASES.contains(&(requested, canonical))
        || requested == "python3"
            && canonical.strip_prefix("python3.").is_some_and(|version| {
                !version.is_empty()
                    && !version.starts_with('.')
                    && !version.ends_with('.')
                    && !version.contains("..")
                    && version
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || byte == b'.')
            })
}

fn bind_std_program(
    command: &std::process::Command,
    canonical: &Path,
    requested_name: &str,
) -> std::process::Command {
    let args = command
        .get_args()
        .map(OsStr::to_os_string)
        .collect::<Vec<_>>();
    let environment = command
        .get_envs()
        .map(|(name, value)| (name.to_os_string(), value.map(OsStr::to_os_string)))
        .collect::<Vec<_>>();
    let environment_cleared = environment.iter().any(|(name, value)| {
        name == OsStr::new(CLEARED_ENV_MARKER) && value.as_deref() == Some(OsStr::new("1"))
    });

    let mut bound = std::process::Command::new(canonical);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        bound.arg0(requested_name);
    }
    bound.args(args);
    if let Some(directory) = command.get_current_dir() {
        bound.current_dir(directory);
    }
    if environment_cleared {
        bound.env_clear();
    }
    for (name, value) in environment {
        if name == OsStr::new(CLEARED_ENV_MARKER) {
            continue;
        }
        if let Some(value) = value {
            bound.env(name, value);
        } else {
            bound.env_remove(name);
        }
    }
    bound
}

fn executable_name(program: &OsStr) -> Option<&str> {
    Path::new(program)
        .file_name()
        .and_then(OsStr::to_str)
        .map(|name| {
            if name.to_ascii_lowercase().ends_with(".exe") {
                &name[..name.len() - 4]
            } else {
                name
            }
        })
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests;
