//! What a rule is allowed to look at, and a record of what it looked at.
//!
//! The corpus declares which inputs are present. A rule asking for more than
//! the corpus holds is withheld rather than run against absent data -- the
//! difference between "no violations" and "nothing to examine" is decided here
//! rather than by each rule remembering to check.

use super::Requires;
use crate::git_manager::diff_context::PrDiffContext;
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
    /// The change under review: base and head revisions, the patch text, and
    /// the changed paths.
    ///
    /// This is [`PrDiffContext`] itself rather than a harness-local retelling
    /// of it. The gates being migrated already take that struct, so a gate
    /// moves into the harness without a translation layer -- and a translation
    /// layer is exactly where the third `Fix` vocabulary came from.
    pub changeset: Option<PrDiffContext>,
    /// Commit subjects the change adds, as `git log base..head` reports them.
    ///
    /// `Option`, and the distinction is the point: `Some(vec![])` is a range
    /// that legitimately adds no non-merge commits, `None` is a log that was
    /// never supplied. Collapsing the two is the defect this harness exists to
    /// make unspellable.
    pub commit_subjects: Option<Vec<String>>,
    /// Whether a resolved build graph was supplied.
    pub build_graph: bool,
    /// Whether a toolchain (cargo, clippy, buck2) is available to invoke.
    pub toolchain: bool,
    /// Whether remote state may be reached.
    pub network: bool,
}

impl Corpus {
    pub fn of_paths(paths: &[&str]) -> Self {
        Corpus {
            subjects: paths.iter().map(|p| Subject::at(p)).collect(),
            ..Default::default()
        }
    }

    /// A corpus over the change a pull request proposes.
    ///
    /// Subjects come from `changed_files`, so a rule that only needs paths runs
    /// against this corpus at the cheapest rung without asking for the diff.
    pub fn of_changeset(ctx: PrDiffContext) -> Self {
        Corpus {
            subjects: ctx.changed_files.iter().map(|p| Subject::at(p)).collect(),
            changeset: Some(ctx),
            ..Default::default()
        }
    }

    /// A corpus over a bare patch, for a rule that reads only the diff text.
    ///
    /// `repo` and `pr_number` are left empty deliberately. They are remote
    /// identity, which is [`Requires::Network`]; a rule declaring
    /// [`Requires::Changeset`] and reading them is lying about its rung, and
    /// leaving them empty is what makes that lie visible instead of silent.
    pub fn of_diff(paths: &[&str], diff: &str) -> Self {
        Corpus::of_changeset(PrDiffContext {
            repo: String::new(),
            pr_number: 0,
            base_branch: String::new(),
            base_sha: String::new(),
            head_sha: String::new(),
            is_incremental: false,
            previous_head_sha: None,
            diff_content: diff.to_string(),
            changed_files: paths.iter().map(|p| p.to_string()).collect(),
            repo_working_dir: std::path::PathBuf::new(),
        })
    }

    pub fn with_commits(mut self, subjects: Vec<String>) -> Self {
        self.commit_subjects = Some(subjects);
        self
    }

    pub fn with_toolchain(mut self) -> Self {
        self.toolchain = true;
        self
    }

    pub fn with_network(mut self) -> Self {
        self.network = true;
        self
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
            Requires::Changeset => self.changeset.is_some(),
            Requires::Manifests => !self.manifests.is_empty(),
            Requires::History => self.commit_subjects.is_some(),
            Requires::BuildGraph => self.build_graph,
            Requires::Toolchain => self.toolchain,
            Requires::Network => self.network,
        }
    }
}
