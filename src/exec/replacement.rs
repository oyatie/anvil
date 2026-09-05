//! Dedicated spawn seam for Anvil's blue/green replacement process.
//!
//! A replacement must outlive the old process, so it cannot use a bounded
//! child runner. Keeping the sole `Command::spawn` here makes that exception a
//! named capability rather than a general escape from the model/non-model
//! transports.

use anyhow::{Result, bail};
use std::path::Path;

#[expect(
    clippy::disallowed_methods,
    reason = "validated blue/green replacement transport owns this execution"
)]
pub(super) fn spawn() -> Result<tokio::process::Child> {
    let mut command = replacement_command()?;
    command
        .spawn()
        .map_err(|error| anyhow::anyhow!("failed to spawn replacement binary: {error}"))
}

fn replacement_command() -> Result<tokio::process::Command> {
    // A handover starts one finite daemon operation. Replaying the ambient
    // argv would repeat whichever state-changing command requested the swap
    // (`review`, `fix`, or `swap --binary`) in the replacement process.
    // There is intentionally no caller-supplied program or argv surface here:
    // `/usr/bin/env agy`, `sh -c agy`, and PATH aliases are all ordinary
    // process launchers, not blue/green replacement capabilities.
    let running = std::env::current_exe()
        .map_err(|error| anyhow::anyhow!("cannot identify the running Anvil binary: {error}"))?;
    let replacement_binary = installed_anvil(&running)?;

    let mut command = tokio::process::Command::new(replacement_binary);
    command.arg("serve");
    #[cfg(unix)]
    command.process_group(0);
    Ok(command)
}

fn installed_anvil(requested: &Path) -> Result<std::path::PathBuf> {
    if super::agent::is_provider_program(requested.as_os_str()) {
        bail!("a model provider cannot be used as Anvil's replacement binary");
    }
    let requested = std::fs::canonicalize(requested).map_err(|error| {
        anyhow::anyhow!(
            "replacement path {} is not a runnable installed Anvil binary: {error}",
            requested.display()
        )
    })?;
    let running = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|error| anyhow::anyhow!("cannot identify the running Anvil binary: {error}"))?;
    if requested != running {
        bail!(
            "replacement path {} is not the installed Anvil binary {}",
            requested.display(),
            running.display()
        );
    }
    let metadata = std::fs::metadata(&requested).map_err(|error| {
        anyhow::anyhow!(
            "cannot inspect installed Anvil binary {}: {error}",
            requested.display()
        )
    })?;
    if !metadata.is_file() {
        bail!(
            "installed Anvil replacement {} is not a regular file",
            requested.display()
        );
    }
    Ok(requested)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejection(path: &Path) -> String {
        installed_anvil(path)
            .expect_err("arbitrary replacement launcher was admitted")
            .to_string()
    }

    #[cfg(unix)]
    #[test]
    fn shell_and_env_launchers_cannot_enter_the_replacement_seam() {
        assert!(rejection(Path::new("/usr/bin/env")).contains("Anvil"));
        assert!(rejection(Path::new("/bin/sh")).contains("Anvil"));
    }

    #[cfg(unix)]
    #[test]
    fn a_path_lookalike_named_anvil_has_no_replacement_authority() {
        use std::os::unix::fs::symlink;

        let scratch = tempfile::tempdir().expect("replacement fixture");
        let alias = scratch.path().join("anvil");
        symlink("/usr/bin/env", &alias).expect("launcher alias");
        assert!(rejection(&alias).contains("Anvil"));
    }

    #[test]
    fn replacement_invocation_is_the_typed_serve_command() {
        let command = replacement_command().expect("construct replacement without launching");
        let command = command.as_std();
        let running = std::env::current_exe()
            .and_then(std::fs::canonicalize)
            .expect("installed executable");
        assert_eq!(command.get_program(), running.as_os_str());
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["serve"]);
    }
}
