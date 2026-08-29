//! ADR-0719 D-8: a directory exists only if something loads it.
//!
//! A predicate, not a denylist — it refuses names nobody anticipated, which is
//! the point.
//!
//! Only build edges and code loads admit. A path named in prose or in inert
//! configuration is recorded as [`Standing::MentionedOnly`] and never counted:
//! writing about a directory is not using it.

use crate::shape::core::tree::TreeSource;
use std::collections::BTreeMap;

/// The unit markers that mean "a build graph reaches this".
///
/// Read from [`LanguageProfile`] rather than spelled here, so the shape
/// program carries no tenant layout of its own.
fn build_markers() -> Vec<&'static str> {
    use crate::shape::core::profile::LanguageProfile;
    vec![
        LanguageProfile::RustCargo.unit_marker(),
        LanguageProfile::RustBuck2.unit_marker(),
    ]
}

/// Files that admit a directory by name rather than by use.
fn owner_law() -> Vec<&'static str> {
    // The build marker comes from the profile for the same reason as
    // `build_markers`: a literal here is a tenant's layout living in the
    // shape program, which is what I13 forbids.
    vec![
        "OWNERS",
        "README.md",
        crate::shape::core::profile::LanguageProfile::RustBuck2.unit_marker(),
        "ADR.md",
        "PRD.md",
        "SPEC.md",
        "PLAN.md",
    ]
}

/// How a directory earns its place, or fails to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// A build graph loads it: a Cargo path dependency or a BUCK target.
    Built { edges: usize },
    /// Code loads it: a path literal in Rust outside comments.
    Loaded { sites: usize },
    /// Owner law: the directory carries only the files D-8 admits by name.
    OwnerLaw,
    /// Named only in prose or inert configuration. Not admissible, and kept
    /// distinct from `Orphan` so a reader knows something still refers to it.
    MentionedOnly { mentions: usize },
    /// Nothing in the tree refers to it at all.
    Orphan,
}

impl Standing {
    /// Whether D-8 admits the directory.
    pub fn admits(&self) -> bool {
        matches!(
            self,
            Standing::Built { .. } | Standing::Loaded { .. } | Standing::OwnerLaw
        )
    }

    /// One line a person can act on.
    pub fn reason(&self) -> String {
        match self {
            Standing::Built { edges } => format!("{edges} build edge(s) load it"),
            Standing::Loaded { sites } => format!("{sites} code site(s) load it"),
            Standing::OwnerLaw => "owner law, admitted by name".to_string(),
            Standing::MentionedOnly { mentions } => format!(
                "{mentions} prose/config mention(s) and no build edge or code load -- \
                 a sentence about a directory is not a loader"
            ),
            Standing::Orphan => "nothing in the tree refers to it".to_string(),
        }
    }
}

/// Where a reference to a path was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Evidence {
    Build,
    Code,
    Prose,
}

fn evidence_kind(path: &str) -> Evidence {
    let base = path.rsplit('/').next().unwrap_or(path);
    // Marker names come from the profile, not from literals here: the shape
    // program must not carry a tenant's layout (I13).
    if build_markers().contains(&base) {
        Evidence::Build
    } else if path.ends_with(".rs") {
        Evidence::Code
    } else {
        Evidence::Prose
    }
}

/// One pass over a tree, answering [`Standing`] for any directory.
///
/// Built once because the scan is the expensive half: every referencing file is
/// read exactly once, and every directory query is then a map lookup.
pub struct LoadIndex {
    /// Directory prefix -> (build edges, code sites, prose mentions).
    refs: BTreeMap<String, (usize, usize, usize)>,
    dirs: Vec<String>,
}

impl LoadIndex {
    pub fn build(tree: &dyn TreeSource, candidates: &[String]) -> Self {
        let mut refs: BTreeMap<String, (usize, usize, usize)> =
            candidates.iter().map(|d| (d.clone(), (0, 0, 0))).collect();

        for path in tree.paths() {
            let kind = evidence_kind(path);
            let Ok(Some(bytes)) = tree.read(path) else {
                continue;
            };
            let Ok(text) = std::str::from_utf8(bytes) else {
                continue;
            };
            // Literals are kept: a runtime path load lives in one.
            let hay = match kind {
                Evidence::Code => crate::source_scan::without_commentary(text),
                _ => text.to_string(),
            };
            for (dir, counts) in refs.iter_mut() {
                if path.starts_with(dir.as_str()) {
                    continue;
                }
                let n = count_refs(&hay, dir);
                if n == 0 {
                    continue;
                }
                match kind {
                    Evidence::Build => counts.0 += n,
                    Evidence::Code => counts.1 += n,
                    Evidence::Prose => counts.2 += n,
                }
            }
        }

        let dirs = candidates.to_vec();
        Self { refs, dirs }
    }

    /// The directories this index was built over.
    pub fn directories(&self) -> &[String] {
        &self.dirs
    }

    /// D-8's verdict for one directory.
    ///
    /// `dir` carries its trailing slash, so `IPs` never matches `IPsomething`.
    pub fn standing(&self, tree: &dyn TreeSource, dir: &str) -> Standing {
        let (build, code, prose) = self.refs.get(dir).copied().unwrap_or((0, 0, 0));
        // Containment admits too: workspace members are closed globs (D-39), so
        // nothing spells a face directory's name even though it holds crates.
        let contains_target = tree.under(dir).iter().any(|p| {
            build_markers()
                .iter()
                .any(|m| p.ends_with(&format!("/{m}")))
        });
        if contains_target {
            return Standing::Built {
                edges: build.max(1),
            };
        }
        if build > 0 {
            return Standing::Built { edges: build };
        }
        if code > 0 {
            return Standing::Loaded { sites: code };
        }
        if is_owner_law_only(tree, dir) {
            return Standing::OwnerLaw;
        }
        if prose > 0 {
            return Standing::MentionedOnly { mentions: prose };
        }
        Standing::Orphan
    }
}

/// References to `dir`, in both spellings a real file uses.
///
/// Manifests and BUCK labels omit the trailing slash, so the bare form counts
/// too — but only where the next character cannot continue a path segment,
/// which keeps `IPs/` from matching `IPsomething/`.
fn count_refs(hay: &str, dir: &str) -> usize {
    let slashed = hay.matches(dir).count();
    let bare = dir.trim_end_matches('/');
    if bare.is_empty() {
        return slashed;
    }
    let mut extra = 0;
    let mut from = 0;
    while let Some(off) = hay[from..].find(bare) {
        let at = from + off;
        let after = at + bare.len();
        from = after;
        // The slashed form is already counted; skip it here.
        if hay[after..].starts_with('/') {
            continue;
        }
        let continues = hay[after..]
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.');
        if !continues {
            extra += 1;
        }
    }
    slashed + extra
}

/// Whether every file directly in `dir` is a name D-8 admits.
///
/// Directly: a nested crate is a different question, and one answered by the
/// build edges rather than here.
fn is_owner_law_only(tree: &dyn TreeSource, dir: &str) -> bool {
    let under = tree.under(dir);
    if under.is_empty() {
        return false;
    }
    under.iter().all(|p| {
        let rest = &p[dir.len()..];
        !rest.contains('/') && owner_law().contains(&rest)
    })
}
