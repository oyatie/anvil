//! D-8 admission, projected over a repository at a revision.
//!
//! Read-only by construction: the tree comes from `git ls-tree` at a named
//! revision, so a dirty working directory cannot affect the answer and nothing
//! is ever written. Remediation for a managed repository is a projection until
//! a person acts on it.

use crate::shape::adapters::git_tree_at_rev::GitTreeAtRev;
use crate::shape::core::load_bearing::{LoadIndex, Standing};
use crate::shape::core::tree::{SourceError, TreeSource};
use std::collections::{BTreeMap, BTreeSet};

/// The four faces D-8 closes a capability to.
pub const FACES: [&str; 4] = ["core", "ports", "adapters", "facade"];

pub struct AdmitRequest {
    pub repo_dir: std::path::PathBuf,
    pub rev: String,
}

#[derive(Debug)]
pub struct DirVerdict {
    pub dir: String,
    pub standing: Standing,
}

#[derive(Debug)]
pub struct AdmitReport {
    pub rev: String,
    pub verdicts: Vec<DirVerdict>,
    /// Crates whose path passes through one of the four faces.
    pub reachable_crates: usize,
    /// Crates that no closed glob over the faces would reach.
    pub unreachable_crates: Vec<String>,
}

impl AdmitReport {
    pub fn refused(&self) -> Vec<&DirVerdict> {
        self.verdicts
            .iter()
            .filter(|v| !v.standing.admits())
            .collect()
    }
}

/// Every first- and second-level directory, as D-8 scopes the rule: the repo
/// root and each capability/app root.
fn candidate_dirs(tree: &dyn TreeSource) -> Vec<String> {
    let mut out: BTreeSet<String> = BTreeSet::new();
    for p in tree.paths() {
        let parts: Vec<&str> = p.split('/').collect();
        if parts.len() >= 2 {
            out.insert(format!("{}/", parts[0]));
        }
        if parts.len() >= 3 {
            out.insert(format!("{}/{}/", parts[0], parts[1]));
        }
    }
    out.into_iter().collect()
}

/// Crate manifests, split by whether a closed glob over the faces reaches them.
fn crate_reachability(tree: &dyn TreeSource) -> (usize, Vec<String>) {
    let mut reachable = 0;
    let mut unreachable = Vec::new();
    for p in tree.paths() {
        let marker = crate::shape::core::profile::LanguageProfile::RustCargo.unit_marker();
        if !p.ends_with(marker) {
            continue;
        }
        let dir = p.trim_end_matches(marker).trim_end_matches('/');
        if dir.is_empty() {
            continue; // the workspace root manifest is not a leaf crate
        }
        if dir.split('/').any(|seg| FACES.contains(&seg)) {
            reachable += 1;
        } else {
            unreachable.push(dir.to_string());
        }
    }
    (reachable, unreachable)
}

pub async fn admit(req: &AdmitRequest) -> Result<AdmitReport, SourceError> {
    // Every text file is read: the evidence for a load lives in manifests,
    // BUCK files and Rust sources alike, and deciding in advance which of them
    // may count is how a scanner acquires a blind spot.
    let tree = GitTreeAtRev::load(&req.repo_dir, &req.rev, |p| {
        let base = p.rsplit('/').next().unwrap_or(p);
        base == crate::shape::core::profile::LanguageProfile::RustBuck2.unit_marker()
            || base == crate::shape::core::profile::LanguageProfile::RustCargo.unit_marker()
            || p.ends_with(".rs")
            || p.ends_with(".md")
    })
    .await?;

    let dirs = candidate_dirs(&tree);
    let index = LoadIndex::build(&tree, &dirs);
    let verdicts = dirs
        .iter()
        .map(|d| DirVerdict {
            dir: d.clone(),
            standing: index.standing(&tree, d),
        })
        .collect();
    let (reachable_crates, unreachable_crates) = crate_reachability(&tree);

    Ok(AdmitReport {
        rev: tree.rev().to_string(),
        verdicts,
        reachable_crates,
        unreachable_crates,
    })
}

pub fn render(r: &AdmitReport, repo: &str) -> String {
    let refused = r.refused();
    let mut s = format!(
        "D-8 admission projection for {repo} @ {}\n\n\
         {} directory(ies) examined, {} refused.\n\
         Crates: {} reachable under closed globs over {:?}, {} unreachable.\n\n",
        &r.rev[..r.rev.len().min(12)],
        r.verdicts.len(),
        refused.len(),
        r.reachable_crates,
        FACES,
        r.unreachable_crates.len(),
    );
    if refused.is_empty() {
        s.push_str("No directory fails D-8's predicate.\n");
        return s;
    }
    // Grouped by leaf name, because remediation is per-name: `IPs/` is one
    // decision taken once, not sixty-three. A flat list of 277 paths truncated
    // to the first 60 hides both the shape of the work and the fact that it
    // was truncated -- and a silent cap reads as "that is all of it".
    let mut by_name: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for v in &refused {
        let leaf = v.dir.trim_end_matches('/').rsplit('/').next().unwrap_or("");
        let e = by_name.entry(leaf).or_insert((0, 0));
        e.0 += 1;
        if matches!(v.standing, Standing::Orphan) {
            e.1 += 1;
        }
    }
    let mut rows: Vec<_> = by_name.into_iter().collect();
    rows.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    s.push_str(&format!(
        "Refused, by directory name ({} distinct name(s)):\n",
        rows.len()
    ));
    s.push_str("  occurrences  orphan  name\n");
    for (name, (n, orphan)) in rows.iter().take(40) {
        s.push_str(&format!("  {n:>11}  {orphan:>6}  {name}\n"));
    }
    if rows.len() > 40 {
        s.push_str(&format!("  ... and {} more name(s)\n", rows.len() - 40));
    }
    s.push_str("\nProjection only. Nothing in the measured repository was written.\n");
    s
}
