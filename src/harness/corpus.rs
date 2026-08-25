//! What a rule is allowed to look at, and a record of what it looked at.
//!
//! The corpus declares which inputs are present. A rule asking for more than
//! the corpus holds is withheld rather than run against absent data -- the
//! difference between "no violations" and "nothing to examine" is decided here
//! rather than by each rule remembering to check.

use super::Requires;
use std::collections::BTreeMap;

/// One thing a rule examines.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Subject {
    /// Repo-relative path.
    pub path: String,
    /// Owning capability or product, when the path resolves to one.
    pub owner: Option<String>,
    /// `core` | `ports` | `adapters` | `facade`, when inside a face.
    pub face: Option<String>,
}

impl Subject {
    pub fn at(path: &str) -> Self {
        let parts: Vec<&str> = path.split('/').collect();
        let face_at = parts
            .iter()
            .position(|s| matches!(*s, "core" | "ports" | "adapters" | "facade"));
        Subject {
            path: path.to_string(),
            owner: face_at
                .and_then(|i| i.checked_sub(1))
                .map(|i| parts[i].to_string()),
            face: face_at.map(|i| parts[i].to_string()),
        }
    }
}

/// The inputs available to this run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Corpus {
    pub subjects: Vec<Subject>,
    /// Path -> contents, for rules needing more than a path.
    pub contents: BTreeMap<String, String>,
    /// Path -> parsed manifest text.
    pub manifests: BTreeMap<String, String>,
    /// Whether a resolved build graph was supplied.
    pub build_graph: bool,
}

impl Corpus {
    pub fn of_paths(paths: &[&str]) -> Self {
        Corpus {
            subjects: paths.iter().map(|p| Subject::at(p)).collect(),
            ..Default::default()
        }
    }

    pub fn with_contents(mut self, path: &str, body: &str) -> Self {
        if !self.subjects.iter().any(|s| s.path == path) {
            self.subjects.push(Subject::at(path));
        }
        self.contents.insert(path.to_string(), body.to_string());
        self
    }

    /// Whether this corpus holds what a rule asked for.
    ///
    /// An empty corpus satisfies nothing: a rule cannot be "clean" over zero
    /// subjects, and this is where that is refused rather than in each rule.
    pub fn satisfies(&self, needs: Requires) -> bool {
        if self.subjects.is_empty() {
            return false;
        }
        match needs {
            Requires::PathsOnly => true,
            Requires::FileContents => !self.contents.is_empty(),
            Requires::Manifests => !self.manifests.is_empty(),
            Requires::BuildGraph => self.build_graph,
        }
    }
}
