//! The prompt on disk, for the one provider that cannot take it on STDIN.
//!
//! `muse exec` reads no prompt from STDIN and refuses `--prompt-file
//! /dev/stdin` as "not a regular file", so its prompt is written to a real
//! file. The alternative is the positional `PROMPT` argument, which puts
//! contributor text into argv, and `ps` makes argv world-readable.
//!
//! It lives beside the transport rather than inside it: the raw stdin seam is
//! censused byte for byte, and a file writer is not part of that seam.

use anyhow::Result;

/// A prompt on disk, created 0600 and unlinked on drop.
pub(super) struct PromptFile(std::path::PathBuf);

impl PromptFile {
    pub(super) fn write(rendered: &str) -> Result<Self> {
        use std::io::Write;
        let name = format!(
            "anvil-prompt-{}-{}.txt",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        );
        let path = std::env::temp_dir().join(name);
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Set with the mode the file is CREATED at, not chmod'ed to
            // afterwards: a window where it is world-readable is the whole
            // reason this is not an argv argument.
            opts.mode(0o600);
        }
        let mut fh = opts
            .open(&path)
            .map_err(|e| anyhow::anyhow!("could not create the prompt file: {e}"))?;
        fh.write_all(rendered.as_bytes())
            .map_err(|e| anyhow::anyhow!("could not write the prompt file: {e}"))?;
        fh.flush()
            .map_err(|e| anyhow::anyhow!("could not flush the prompt file: {e}"))?;
        Ok(Self(path))
    }

    pub(super) fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for PromptFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
