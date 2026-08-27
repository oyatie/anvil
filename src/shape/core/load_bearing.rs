//! D-8's admission test: a directory exists only if something loads it.
//!
//! # The rule, verbatim
//!
//! > "Closed directory set for repo root and capability/app/<product>/ roots.
//! > A name exists only if a compiler, test, PDP, SLO controller, or reconciler
//! > loads it (or it is OWNERS/README/BUCK or ADR.md PRD.md SPEC.md PLAN.md on
//! > the owner)."
//!
//! It is a PREDICATE, not a denylist. That distinction is the whole value: a
//! denylist only ever refuses names somebody thought to write down, so `IPs/`
//! is refused and `ip-registry/` is not. A predicate refuses every name nobody
//! loads, including the ones nobody anticipated.
//!
//! # Why a string match is not a load
//!
//! A first census of one repository counted "files referencing `specs/`" and
//! got 247. That number is an upper bound on loads and a poor one: a path
//! written in a markdown sentence, a rationale field in a planning JSON, or a
//! comment counts the same as a `path = "..."` dependency. Acting on it would
//! keep every doomed directory alive on the strength of prose about deleting
//! it -- and in the repository this was written for, the only thing referring
//! to `IPs/` outside `IPs/` was a planning file describing the plan to create
//! them.
//!
//! So [`Standing`] separates evidence by kind and only build edges and code
//! loads admit. Prose is recorded, never counted.

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
    /// Named only in prose or in configuration nothing executes.
    ///
    /// Deliberately NOT admissible, and deliberately distinct from `Orphan`:
    /// a reader deciding whether to delete needs to know that something,
    /// somewhere, still talks about it.
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
    // Marker names come from the profile, not from literals here. Anvil's
    // own I13 guard refused the first draft of this module for spelling
    // `Cargo.toml` and `BUCK` inline: the shape program must not carry a
    // tenant's layout, and `LanguageProfile::unit_marker` already held both.
    if build_markers().iter().any(|m| base == *m) {
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
            // Comments are stripped for Rust; string literals are KEPT,
            // because a runtime path load lives in one. `code_only` would
            // remove exactly the evidence being looked for.
            let hay = match kind {
                Evidence::Code => crate::source_scan::without_commentary(text),
                _ => text.to_string(),
            };
            for (dir, counts) in refs.iter_mut() {
                // A directory does not load itself.
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
        // A directory that CONTAINS a build target is load-bearing whether or
        // not anything spells its name. Workspace members are closed globs by
        // D-39, and a BUCK label is `//billing/facade/invoicing:lib` written
        // inside that directory -- which the self-reference guard correctly
        // skips. Counting only inbound references therefore refused
        // `billing/facade/`, a face holding real crates, on the first run of
        // this projection. Containment is the missing half of the predicate,
        // and it is the half that decides whether the rule condemns the tree
        // it is meant to protect.
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

/// References to `dir` in `hay`, counting both spellings a real file uses.
///
/// A directory key carries its trailing slash so `IPs/` cannot match
/// `IPsomething/`. But a Cargo manifest writes `path = "storage/core/blob"`
/// and a BUCK label writes `//storage/core/blob:lib`, neither with a trailing
/// slash -- so keying only on the slashed form scores a real build edge as
/// zero, which is the false-negative half of this rule and the worse half:
/// it deletes a directory something builds.
///
/// The unslashed form is therefore matched too, but only where the next
/// character cannot continue a path segment. That keeps the prefix collision
/// out without reintroducing it through the back door.
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
