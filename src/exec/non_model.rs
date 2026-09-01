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
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Output};
use std::time::Duration;
use tokio::process::Command;

use super::{ExecClass, agent};

const NON_MODEL_PROGRAMS: &[&str] = &[
    "cargo", "cedar", "curl", "echo", "gh", "git", "go", "node", "npm", "ps", "python3", "sleep",
];

/// A raw command that has passed the finite non-model admission policy.
/// Its inner command never leaves this module.
struct NonModelCommand(Command);

/// Synchronous counterpart for call chains and destructors that cannot await.
struct SyncNonModelCommand(std::process::Command);

struct ResolvedExecutable {
    canonical: PathBuf,
    locked_path: Option<OsString>,
}

impl NonModelCommand {
    fn checked(mut command: Command) -> Result<Self> {
        let resolved = validate_program(&command)?;
        if let Some(path) = resolved.locked_path {
            command.env("PATH", path);
        }
        Ok(Self(command))
    }
}

impl SyncNonModelCommand {
    fn checked(mut command: std::process::Command) -> Result<Self> {
        let resolved = validate_std_program(&command)?;
        if let Some(path) = resolved.locked_path {
            command.env("PATH", path);
        }
        Ok(Self(command))
    }
}

mod transport;

pub(super) async fn run(command: Command, class: ExecClass, what: &str) -> Result<Output> {
    let command = NonModelCommand::checked(command)?;
    transport::run_for(command, class.timeout(), what, Some(class)).await
}

pub(super) async fn run_for(command: Command, limit: Duration, what: &str) -> Result<Output> {
    let command = NonModelCommand::checked(command)?;
    transport::run_for(command, limit, what, None).await
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

fn validate_program(command: &Command) -> Result<ResolvedExecutable> {
    validate_std_program(command.as_std())
}

fn validate_std_program(command: &std::process::Command) -> Result<ResolvedExecutable> {
    let requested = command.get_program();
    let requested_name = executable_name(requested)
        .ok_or_else(|| anyhow::anyhow!("raw subprocess program has no valid executable name"))?;

    if agent::is_provider_program(requested) {
        bail!("model provider commands require the typed AgentCommand + ModelPrompt transport");
    }

    if !NON_MODEL_PROGRAMS.contains(&requested_name) {
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
    Ok(resolved)
}

fn executable_name(program: &OsStr) -> Option<&str> {
    Path::new(program)
        .file_name()
        .and_then(OsStr::to_str)
        .map(|name| name.trim_end_matches(".exe"))
        .filter(|name| !name.is_empty())
}

fn resolve_executable(command: &std::process::Command) -> Option<ResolvedExecutable> {
    let program = Path::new(command.get_program());
    if program.components().count() > 1 {
        let candidate = if program.is_absolute() {
            program.to_path_buf()
        } else {
            command
                .get_current_dir()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(program)
        };
        return runnable_canonical(&candidate).map(|canonical| ResolvedExecutable {
            canonical,
            locked_path: None,
        });
    }

    let explicit_path = command.get_envs().find_map(|(name, value)| {
        (name == OsStr::new("PATH")).then(|| value.map(OsStr::to_os_string))?
    });
    let search_path: OsString = explicit_path.or_else(|| std::env::var_os("PATH"))?;
    for dir in std::env::split_paths(&search_path) {
        let effective_dir = if dir.is_absolute() {
            dir
        } else {
            command
                .get_current_dir()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
                .join(dir)
        };
        let Some((candidate, canonical)) = runnable_in(&effective_dir, program) else {
            continue;
        };
        let selected_dir = std::fs::canonicalize(candidate.parent()?).ok()?;
        let locked_path = std::env::join_paths(
            std::iter::once(selected_dir).chain(std::env::split_paths(&search_path)),
        )
        .ok()?;
        return Some(ResolvedExecutable {
            canonical,
            locked_path: Some(locked_path),
        });
    }
    None
}

fn runnable_in(directory: &Path, program: &Path) -> Option<(PathBuf, PathBuf)> {
    let candidate = directory.join(program);
    if let Some(canonical) = runnable_canonical(&candidate) {
        return Some((candidate, canonical));
    }
    #[cfg(windows)]
    if program.extension().is_none() {
        for extension in ["exe", "com", "bat", "cmd"] {
            let candidate = directory.join(program).with_extension(extension);
            if let Some(canonical) = runnable_canonical(&candidate) {
                return Some((candidate, canonical));
            }
        }
    }
    None
}

fn runnable_canonical(candidate: &Path) -> Option<PathBuf> {
    let canonical = std::fs::canonicalize(candidate).ok()?;
    let metadata = std::fs::metadata(&canonical).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(canonical)
}

#[cfg(test)]
mod tests;
