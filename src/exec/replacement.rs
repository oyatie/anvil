//! Dedicated spawn seam for Anvil's blue/green replacement process.
//!
//! A replacement must outlive the old process, so it cannot use a bounded
//! child runner. Keeping the sole `Command::spawn` here makes that exception a
//! named capability rather than a general escape from the model/non-model
//! transports.

use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn spawn(replacement_binary: &Path, args: &[String]) -> Result<tokio::process::Child> {
    if super::agent::is_provider_program(replacement_binary.as_os_str())
        || std::fs::canonicalize(replacement_binary)
            .ok()
            .is_some_and(|resolved| super::agent::is_provider_program(resolved.as_os_str()))
    {
        bail!("a model provider cannot be used as Anvil's replacement binary");
    }

    let mut command = tokio::process::Command::new(replacement_binary);
    command.args(args);
    #[cfg(unix)]
    command.process_group(0);
    command
        .spawn()
        .map_err(|error| anyhow::anyhow!("failed to spawn replacement binary: {error}"))
}
