//! Reads a tree at a named revision with git plumbing only.
//!
//! `git ls-tree -r --name-only <rev>` lists the paths; one
//! `git cat-file --batch` call loads every file the caller selected. Nothing
//! is checked out, so the measurement is of the commit, not of whatever a
//! shared clone happens to have on disk (I3, G1) — and the same primitive
//! reads the frozen reference at a merge-base.

use super::InMemoryTree;
use crate::exec::{ExecClass, run_bounded, run_bounded_with_stdin};
use crate::shape::ports::SourceError;
use std::collections::BTreeMap;
use std::path::Path;
use tokio::process::Command;

pub struct GitTreeAtRev;

impl GitTreeAtRev {
    /// Resolves `rev` to a full sha (so a report never names a moving ref),
    /// lists the tree, and loads the files `select` accepts.
    pub async fn load(
        repo_dir: &Path,
        rev: &str,
        select: impl Fn(&str) -> bool,
    ) -> Result<InMemoryTree, SourceError> {
        let sha = Self::resolve(repo_dir, rev).await?;

        let mut ls = Command::new("git");
        ls.current_dir(repo_dir)
            .args(["ls-tree", "-r", "--name-only", "-z", &sha]);
        let out = run_bounded(ls, ExecClass::Vcs, "git ls-tree (shape)")
            .await
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        if !out.status.success() {
            return Err(SourceError::Unavailable(format!(
                "git ls-tree {sha} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        let mut paths: Vec<String> = out
            .stdout
            .split(|b| *b == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).to_string())
            .collect();
        paths.sort();

        let wanted: Vec<&String> = paths.iter().filter(|p| select(p)).collect();
        let files = Self::read_batch(repo_dir, &sha, &wanted).await?;
        Ok(InMemoryTree::new(&sha, paths, files))
    }

    pub async fn resolve(repo_dir: &Path, rev: &str) -> Result<String, SourceError> {
        let mut cmd = Command::new("git");
        cmd.current_dir(repo_dir)
            .args(["rev-parse", "--verify", &format!("{rev}^{{commit}}")]);
        let out = run_bounded(cmd, ExecClass::Quick, "git rev-parse (shape)")
            .await
            .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        if !out.status.success() {
            return Err(SourceError::Unavailable(format!(
                "{rev} is not a commit in {}: {}",
                repo_dir.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    async fn read_batch(
        repo_dir: &Path,
        sha: &str,
        paths: &[&String],
    ) -> Result<BTreeMap<String, Vec<u8>>, SourceError> {
        let mut files = BTreeMap::new();
        if paths.is_empty() {
            return Ok(files);
        }
        let payload: String = paths.iter().map(|p| format!("{sha}:{p}\n")).collect();
        let mut cmd = Command::new("git");
        cmd.current_dir(repo_dir).args(["cat-file", "--batch"]);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        let out = run_bounded_with_stdin(
            cmd,
            &payload,
            ExecClass::Vcs.timeout(),
            "git cat-file --batch (shape)",
        )
        .await
        .map_err(|e| SourceError::Unavailable(e.to_string()))?;
        if !out.status.success() {
            return Err(SourceError::Unavailable(format!(
                "git cat-file --batch failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            )));
        }
        Self::parse_batch(&out.stdout, paths, &mut files)?;
        Ok(files)
    }

    /// `<sha> blob <size>\n<bytes>\n` per hit, `<spec> missing\n` per miss,
    /// in request order.
    fn parse_batch(
        bytes: &[u8],
        order: &[&String],
        into: &mut BTreeMap<String, Vec<u8>>,
    ) -> Result<(), SourceError> {
        let mut pos = 0usize;
        for path in order {
            let nl = bytes[pos..]
                .iter()
                .position(|b| *b == b'\n')
                .ok_or_else(|| SourceError::Unavailable("truncated cat-file output".into()))?;
            let header = String::from_utf8_lossy(&bytes[pos..pos + nl]).to_string();
            pos += nl + 1;
            let parts: Vec<&str> = header.split(' ').collect();
            if parts.len() == 2 && parts[1] == "missing" {
                continue;
            }
            if parts.len() != 3 {
                return Err(SourceError::Unavailable(format!(
                    "unexpected cat-file header {header:?}"
                )));
            }
            let size: usize = parts[2]
                .parse()
                .map_err(|_| SourceError::Unavailable(format!("bad size in {header:?}")))?;
            if pos + size > bytes.len() {
                return Err(SourceError::Unavailable("truncated cat-file blob".into()));
            }
            into.insert(path.to_string(), bytes[pos..pos + size].to_vec());
            pos += size + 1; // trailing newline after the blob
        }
        Ok(())
    }
}
