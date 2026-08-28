//! D-8 admission, projected over a repository at a revision.
//!
//! Read-only by construction: the tree comes from `git ls-tree` at a named
//! revision, so a dirty working directory cannot affect the answer and nothing
//! is ever written. Remediation for a managed repository is a projection until
//! a person acts on it.

use crate::shape::adapters::git_tree_at_rev::GitTreeAtRev;
use crate::shape::ports::{LoadIndex, SourceError, Standing, TreeSource};
use std::collections::{BTreeMap, BTreeSet};

/// The directories Cargo gives a crate, relative to its manifest. Below these
/// the compiler and D-30/D-35 decide; D-8 does not.
const CRATE_SOURCE_DIRS: [&str; 4] = ["src/", "tests/", "benches/", "examples/"];

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

/// Every directory D-8 governs: those OUTSIDE any crate.
///
/// Not the first two levels: a directory nothing loads sits at any depth, and
/// a projection that silently covers a fifth of the tree reads as the whole.
///
/// Not "every directory" either. Inside a crate, `src/` and its module
/// directories are governed by the compiler and by D-30/D-35, and nothing
/// references them by path -- so an unbounded predicate would refuse `src/`
/// itself. The boundary is the nearest enclosing build target: above it D-8
/// decides, below it the crate does.
fn candidate_dirs(tree: &dyn TreeSource) -> Vec<String> {
    // The CARGO marker only. A Buck2 BUCK file is a package marker, not a
    // crate boundary -- every directory may carry one, and this repository has
    // thirty at capability level (`audit/BUCK`, `billing/BUCK`, ...) against
    // two Cargo.toml. Treating BUCK as the boundary excluded every capability's
    // children and silently hid all sixty-three `IPs/` directories from the
    // projection: a report that had stopped looking, reading as a report that
    // had found nothing.
    let markers = [crate::shape::ports::LanguageProfile::RustCargo.unit_marker()];
    // Directories that ARE a crate root.
    // A manifest at the repository root has no `/` to split on. Deriving the
    // crate root by splitting alone dropped it, so a single-crate repository
    // registered no boundary, excluded nothing, and judged the crate's own
    // module directories as if each were a capability -- 66 of them on anvil,
    // every one `Orphan`, because a Rust module is loaded by `mod name;` and
    // never by a path literal. oyatie never showed it: its manifests are all
    // nested, so the split always succeeded.
    let crate_roots: BTreeSet<String> = tree
        .paths()
        .iter()
        .filter_map(|p| {
            let (dir, base) = match p.rsplit_once('/') {
                Some((dir, base)) => (format!("{dir}/"), base),
                None => (String::new(), p.as_str()),
            };
            markers.contains(&base).then_some(dir)
        })
        .collect();

    let mut all: BTreeSet<String> = BTreeSet::new();
    for p in tree.paths() {
        let mut acc = String::new();
        for seg in p.split('/').rev().skip(1).collect::<Vec<_>>().iter().rev() {
            acc.push_str(seg);
            acc.push('/');
            all.insert(acc.clone());
        }
    }
    all.into_iter()
        .filter(|d| {
            // Excluded if any STRICT ancestor is a crate root. The directory
            // itself being a crate root is fine -- that is the crate D-8
            // placed, and it is admitted by containment anyway.
            //
            // The repository root is the one crate root that cannot be read
            // this way. Everything is strictly below it, so the ancestor test
            // would exclude the whole tree -- including the top-level
            // directories whose closed set is precisely what D-8 decides. What
            // a root-level crate governs is its source tree, so that is what
            // it withholds.
            !crate_roots.iter().any(|r| {
                if r.is_empty() {
                    CRATE_SOURCE_DIRS.iter().any(|src| d.starts_with(src))
                } else {
                    r != d && d.starts_with(r.as_str())
                }
            })
        })
        .collect()
}

/// Crate manifests, split by whether a closed glob over the faces reaches them.
fn crate_reachability(tree: &dyn TreeSource) -> (usize, Vec<String>) {
    let mut reachable = 0;
    let mut unreachable = Vec::new();
    for p in tree.paths() {
        let marker = crate::shape::ports::LanguageProfile::RustCargo.unit_marker();
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
        base == crate::shape::ports::LanguageProfile::RustBuck2.unit_marker()
            || base == crate::shape::ports::LanguageProfile::RustCargo.unit_marker()
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
