//! Reads the frozen reference at `git merge-base <base> <head>`.
//!
//! The base ref comes from the pull request context (the branch the change
//! targets), not from anything inside the change, so a change cannot point
//! the ratchet at a reference of its own choosing.

use crate::exec::{ExecClass, run_bounded};
use crate::ratchet::ports::{FrozenReferenceSource, RefError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokio::process::Command;

pub struct GitMergeBase {
    repo_dir: PathBuf,
    merge_base: String,
    cache: Mutex<BTreeMap<String, Option<Vec<u8>>>>,
}

impl GitMergeBase {
    /// Resolves the merge-base up front so every read is against one commit.
    pub async fn resolve(repo_dir: &Path, base_ref: &str, head: &str) -> Result<Self, RefError> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo_dir)
            .args(["merge-base", base_ref, head]);
        let out = run_bounded(cmd, ExecClass::Quick, "git merge-base (ratchet)")
            .await
            .map_err(|e| RefError::Unavailable(e.to_string()))?;
        if !out.status.success() {
            return Err(RefError::Unavailable(format!(
                "git merge-base {base_ref} {head} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let merge_base = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if merge_base.len() != 40 {
            return Err(RefError::Unavailable(format!(
                "merge-base did not resolve to a sha: {merge_base:?}"
            )));
        }
        Ok(GitMergeBase {
            repo_dir: repo_dir.to_path_buf(),
            merge_base,
            cache: Mutex::new(BTreeMap::new()),
        })
    }

    /// Loads `paths` at the merge-base into the cache; later `read`s are
    /// synchronous. Absent paths are cached as `None`.
    pub async fn preload(&self, paths: &[&str]) -> Result<(), RefError> {
        for path in paths {
            let mut cmd = Command::new("git");
            cmd.current_dir(&self.repo_dir)
                .args(["show", &format!("{}:{path}", self.merge_base)]);
            let out = run_bounded(cmd, ExecClass::Quick, "git show (ratchet reference)")
                .await
                .map_err(|e| RefError::Unavailable(e.to_string()))?;
            let value = if out.status.success() {
                Some(out.stdout)
            } else {
                None
            };
            self.cache
                .lock()
                .map_err(|_| RefError::Unavailable("reference cache poisoned".into()))?
                .insert(path.to_string(), value);
        }
        Ok(())
    }
}

impl FrozenReferenceSource for GitMergeBase {
    fn reference_rev(&self) -> &str {
        &self.merge_base
    }

    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, RefError> {
        let cache = self
            .cache
            .lock()
            .map_err(|_| RefError::Unavailable("reference cache poisoned".into()))?;
        match cache.get(path) {
            Some(v) => Ok(v.clone()),
            None => Err(RefError::Unavailable(format!(
                "{path} was not preloaded from {}",
                self.merge_base
            ))),
        }
    }
}
