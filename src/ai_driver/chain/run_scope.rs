//! A stage's declared write scope, on disk for the length of one turn.
//!
//! `pre-commit` reads `.anvil/run-scope` and refuses staged paths outside it.
//! Nothing wrote that file until this existed, so the guardrail could not fire
//! in a real checkout (#215).
//!
//! Its own module for the same reason `prompt_file` is: `chain.rs` decides which
//! model serves a stage, and a file writer is not that decision.

use anyhow::{Context, Result};
use std::path::Path;

/// A stage's write scope, declared on disk for the length of one turn.
///
/// Removed on drop so ordinary work outside a run sees no constraint at all --
/// the hook treats an absent declaration as "not a milestone run", which is
/// different from an empty one meaning "may write nothing".
#[must_use = "the scope is released the moment this is dropped; hold it across the commit"]
pub struct RunScope {
    path: std::path::PathBuf,
    dir_was_created: bool,
}

impl RunScope {
    pub fn declare(working_dir: &Path, writes: &[String]) -> Result<Self> {
        let dir = working_dir.join(".anvil");
        let dir_was_created = !dir.exists();
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("could not create {}", dir.display()))?;
        let path = dir.join("run-scope");
        // `create_new`: a declaration already present is another run in this
        // workspace, and silently overwriting its scope would widen or narrow a
        // constraint it is relying on.
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .with_context(|| {
                format!(
                    "could not declare the run scope at {}; a declaration already present means \
                     another run holds this workspace",
                    path.display()
                )
            })?;
        use std::io::Write;
        for prefix in writes {
            writeln!(file, "{prefix}")
                .with_context(|| format!("could not write the run scope at {}", path.display()))?;
        }
        file.flush()
            .with_context(|| format!("could not flush the run scope at {}", path.display()))?;
        Ok(Self {
            path,
            dir_was_created,
        })
    }
}

impl Drop for RunScope {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
        if self.dir_was_created {
            // Only the directory this run created, and only if nothing else
            // landed in it.
            if let Some(dir) = self.path.parent() {
                let _ = std::fs::remove_dir(dir);
            }
        }
    }
}
