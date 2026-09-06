//! Only known absence is bootstrap; existing unreadable or invalid input fails.

use std::{io, path::Path};

use crate::ratchet::facade::{Baseline, Signoff};
use anyhow::{Context, Result, anyhow, bail};

pub(super) struct Inputs {
    pub previous: Option<Baseline>,
    pub signoff: Signoff,
}

fn existing_regular(observation: io::Result<bool>) -> Result<bool> {
    match observation {
        Ok(true) => Ok(true),
        Ok(false) => bail!("Existing input must be an ordinary regular file"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

async fn read_optional_regular(path: &Path) -> Result<Option<Vec<u8>>> {
    // This classifies the final entry, not atomic custody of its ancestors.
    let observed = tokio::fs::symlink_metadata(path).await.map(|m| m.is_file());
    if !existing_regular(observed).with_context(|| format!("Inspecting {}", path.display()))? {
        return Ok(None);
    }
    // Once presence is observed, even a subsequent NotFound remains a failure.
    after_presence(tokio::fs::read(path).await)
        .with_context(|| format!("Reading existing input {}", path.display()))
}

fn after_presence(read: io::Result<Vec<u8>>) -> Result<Option<Vec<u8>>> {
    Ok(Some(read?))
}

pub(super) async fn load(out: Option<&Path>, signoff_path: &Path) -> Result<Inputs> {
    let previous = match out {
        Some(path) => read_optional_regular(path)
            .await?
            .map(|bytes| {
                Baseline::parse(&bytes)
                    .map_err(|e| anyhow!(e))
                    .with_context(|| format!("Parsing existing baseline {}", path.display()))
            })
            .transpose()?,
        None => None,
    };
    let signoff = read_optional_regular(signoff_path)
        .await?
        .map(|bytes| {
            Signoff::parse(&bytes)
                .map_err(|e| anyhow!(e))
                .with_context(|| format!("Parsing existing signoff {}", signoff_path.display()))
        })
        .transpose()?
        .unwrap_or_default();
    Ok(Inputs { previous, signoff })
}

#[cfg(test)]
mod tests;
