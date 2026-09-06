use super::{AgentCommand, Framing, Posture, provider};
use anyhow::{Result, bail};
use tokio::process::Command;

/// [`super::command`], against a stated environment. Used by posture unit tests.
pub(super) fn command_in<I>(
    tool: &str,
    posture: &Posture,
    framing: Framing,
    environment: I,
) -> Result<AgentCommand>
where
    I: IntoIterator<Item = (String, String)>,
{
    let cmd = trusted_provider_command(tool)?;
    Ok(prepare_command(cmd, posture, framing, environment))
}

pub(super) fn prepare_command<I>(
    mut cmd: Command,
    posture: &Posture,
    framing: Framing,
    environment: I,
) -> AgentCommand
where
    I: IntoIterator<Item = (String, String)>,
{
    posture.apply_from(&mut cmd, environment);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    AgentCommand {
        command: cmd,
        framing,
    }
}

pub(super) fn trusted_provider_command(tool: &str) -> Result<Command> {
    if !provider::is_provider_program(std::ffi::OsStr::new(tool)) {
        bail!("{tool:?} is outside the finite provider executable registry");
    }
    let search_path = std::env::var_os("PATH")
        .ok_or_else(|| anyhow::anyhow!("service PATH is absent while resolving provider {tool}"))?;
    trusted_provider_command_from(tool, &search_path)
}

/// Resolves and binds the provider using one captured service-PATH value.
///
/// The resulting absolute pathname is the deployment's trust boundary. This
/// seam does not claim to stop the service owner from replacing that installed
/// file after resolution and before the OS opens it; filesystem containment is
/// explicitly outside the direct-turn contract.
pub(super) fn trusted_provider_command_from(
    tool: &str,
    search_path: &std::ffi::OsStr,
) -> Result<Command> {
    if std::env::split_paths(search_path).any(|directory| !directory.is_absolute()) {
        bail!("service PATH contains a relative entry; provider identity is not trustworthy");
    }
    let mut requested = std::process::Command::new(tool);
    requested.env_clear().env("PATH", search_path);
    let canonical = crate::exec::non_model::resolution::resolve_canonical_executable(&requested)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "provider executable {tool:?} is unavailable on the trusted service PATH"
            )
        })?;
    let mut bound = std::process::Command::new(canonical);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        bound.arg0(tool);
    }
    Ok(Command::from(bound))
}
