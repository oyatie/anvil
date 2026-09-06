//! A tree held entirely in memory. Fixtures build it directly; the git
//! adapter builds it from plumbing output so the core never blocks on IO.

use crate::shape::ports::{SourceError, TreeSource};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InMemoryTree {
    rev: String,
    paths: Vec<String>,
    files: BTreeMap<String, Vec<u8>>,
}

impl InMemoryTree {
    pub fn new(rev: &str, mut paths: Vec<String>, files: BTreeMap<String, Vec<u8>>) -> Self {
        for p in &files {
            if !paths.contains(p.0) {
                paths.push(p.0.clone());
            }
        }
        paths.sort();
        paths.dedup();
        InMemoryTree {
            rev: rev.to_string(),
            paths,
            files,
        }
    }

    /// Fixture helper: paths only, no contents.
    pub fn from_paths(rev: &str, paths: &[&str]) -> Self {
        Self::new(
            rev,
            paths.iter().map(|p| p.to_string()).collect(),
            BTreeMap::new(),
        )
    }

    /// Fixture helper: adds a file with contents (and its path).
    pub fn with_file(mut self, rel: &str, contents: &str) -> Self {
        self.files
            .insert(rel.to_string(), contents.as_bytes().to_vec());
        if !self.paths.iter().any(|p| p == rel) {
            self.paths.push(rel.to_string());
            self.paths.sort();
        }
        self
    }
}

impl TreeSource for InMemoryTree {
    fn rev(&self) -> &str {
        &self.rev
    }

    fn paths(&self) -> &[String] {
        &self.paths
    }

    fn read(&self, rel: &str) -> Result<Option<&[u8]>, SourceError> {
        if let Some(bytes) = self.files.get(rel) {
            return Ok(Some(bytes.as_slice()));
        }
        if self.contains(rel) {
            return Err(SourceError::NotLoaded(rel.to_string()));
        }
        Ok(None)
    }

    fn loaded(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
}
