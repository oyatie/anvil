//! Target identity evidence for architecture checks, not a Rust name resolver.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{declaration::exact_production_roles, read_parsed, roots::architecture_roots};
use crate::source_scan::cfg::excludes_when_test_is_false;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CrateIdentity {
    pub(super) manifest: Option<PathBuf>,
    pub(super) root: PathBuf,
    pub(super) kind: &'static str,
}

pub(super) struct OwnershipRoot {
    pub(super) identity: CrateIdentity,
    pub(super) aliases: BTreeMap<String, BTreeSet<CrateIdentity>>,
}

struct TargetEvidence {
    root: OwnershipRoot,
    files: BTreeSet<PathBuf>,
    self_aliases: BTreeSet<String>,
    uncertain_aliases: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RootRelation {
    SameCrate,
    OtherCrate,
    Foreign,
    Unknown,
}

pub(crate) struct ArchitectureOwnership {
    repo: PathBuf,
    targets: Vec<TargetEvidence>,
}

impl ArchitectureOwnership {
    pub(crate) fn new(repo: &Path) -> Result<Self, String> {
        let repo = fs::canonicalize(repo)
            .map_err(|error| format!("cannot resolve architecture repository: {error}"))?;
        let mut targets = Vec::new();
        for root in architecture_roots(&repo)? {
            let files = exact_production_roles(&repo, &root.identity.root)?;
            let (_, syntax) = read_parsed(&root.identity.root)?;
            let (self_aliases, uncertain_aliases) = self_aliases(&syntax);
            targets.push(TargetEvidence {
                root,
                files,
                self_aliases,
                uncertain_aliases,
            });
        }
        Ok(Self { repo, targets })
    }

    pub(crate) fn relation(&self, importing_file: &str, name: &str) -> RootRelation {
        // `crate` is a same-target spelling independent of manifest names.
        if name == "crate" {
            return RootRelation::SameCrate;
        }
        let path = self.repo.join(importing_file);
        let Ok(path) = fs::canonicalize(path) else {
            return RootRelation::Unknown;
        };
        if !path.starts_with(&self.repo) {
            return RootRelation::Unknown;
        }
        self.relation_at(&path, name)
    }

    fn relation_at(&self, path: &Path, name: &str) -> RootRelation {
        let mut result = RootRelation::Foreign;
        let mut owners = 0;
        let mut unknown = false;
        for target in self
            .targets
            .iter()
            .filter(|target| target.files.contains(path))
        {
            owners += 1;
            if target.uncertain_aliases.contains(name) {
                unknown = true;
                continue;
            }
            let self_alias = target.self_aliases.contains(name);
            if self_alias {
                // An explicit root binding names this crate; a same-spelled
                // dependency in the extern prelude does not replace it.
                result = RootRelation::SameCrate;
            } else if let Some(destinations) = target.root.aliases.get(name) {
                if destinations.len() != 1 {
                    unknown = true;
                } else if destinations.contains(&target.root.identity) {
                    result = RootRelation::SameCrate;
                } else {
                    // One definite other-target context suffices; uncertainty
                    // elsewhere must not erase its concrete forbidden edge.
                    return RootRelation::OtherCrate;
                }
            }
        }
        if owners == 0 || unknown {
            RootRelation::Unknown
        } else {
            result
        }
    }
}

fn self_aliases(syntax: &syn::File) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut known = BTreeSet::new();
    let mut uncertain = BTreeSet::new();
    for item in &syntax.items {
        let syn::Item::ExternCrate(item) = item else {
            continue;
        };
        if item.ident != "self" || excludes_when_test_is_false(&item.attrs) {
            continue;
        }
        let Some((_, alias)) = &item.rename else {
            continue;
        };
        if item
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
        {
            uncertain.insert(alias.to_string());
        } else {
            known.insert(alias.to_string());
        }
    }
    (known, uncertain)
}
