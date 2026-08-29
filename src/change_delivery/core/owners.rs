//! Ownership resolution: GitHub CODEOWNERS (last matching pattern wins) plus
//! nearest-ancestor OWNERS files. Owners decide which shards may travel
//! together: two shards whose owners intersect are not independent.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnerMap {
    /// CODEOWNERS rules in file order: (pattern, owners).
    codeowners: Vec<(String, BTreeSet<String>)>,
    /// Directory (with trailing slash, "" for root) -> owners from its OWNERS file.
    owners_files: BTreeMap<String, BTreeSet<String>>,
}

impl OwnerMap {
    pub fn from_codeowners(text: &str) -> OwnerMap {
        let mut m = OwnerMap::default();
        m.add_codeowners(text);
        m
    }

    pub fn add_codeowners(&mut self, text: &str) {
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.split_whitespace();
            let Some(pattern) = parts.next() else {
                continue;
            };
            let owners: BTreeSet<String> = parts.map(str::to_string).collect();
            self.codeowners.push((pattern.to_string(), owners));
        }
    }

    /// `dir` is the directory holding the OWNERS file, "" for the root.
    pub fn add_owners_file(&mut self, dir: &str, text: &str) {
        let owners: BTreeSet<String> = text
            .lines()
            .map(|l| l.split('#').next().unwrap_or("").trim())
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        let key = if dir.is_empty() || dir.ends_with('/') {
            dir.to_string()
        } else {
            format!("{dir}/")
        };
        self.owners_files.insert(key, owners);
    }

    /// Partitioning owners of `path`: the most specific CODEOWNERS rule that
    /// is not the `*` catch-all, plus the nearest non-root OWNERS file. A
    /// catch-all owns everything and therefore partitions nothing; counting
    /// it made every pair of shards conflict on oyatie (3240 pairs).
    pub fn owners_of(&self, path: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for (pattern, owners) in self.codeowners.iter().rev() {
            if pattern == "*" {
                continue;
            }
            if codeowners_matches(pattern, path) {
                out.extend(owners.iter().cloned());
                break;
            }
        }
        // Nearest OWNERS file up the tree, stopping before the root.
        let mut dir = path
            .rsplit_once('/')
            .map(|(d, _)| format!("{d}/"))
            .unwrap_or_default();
        while !dir.is_empty() {
            if let Some(o) = self.owners_files.get(&dir) {
                out.extend(o.iter().cloned());
                break;
            }
            let trimmed = dir.trim_end_matches('/');
            dir = trimmed
                .rsplit_once('/')
                .map(|(d, _)| format!("{d}/"))
                .unwrap_or_default();
        }
        out
    }

    /// Who answers for `path` when nobody specific does: the `*` rule and the
    /// root OWNERS file. Used for review routing, never for conflicts.
    pub fn fallback_owners(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for (pattern, owners) in &self.codeowners {
            if pattern == "*" {
                out.extend(owners.iter().cloned());
            }
        }
        if let Some(o) = self.owners_files.get("") {
            out.extend(o.iter().cloned());
        }
        out
    }
}

/// CODEOWNERS pattern semantics (gitignore-like): `*` everything; a pattern
/// ending in `/` matches the directory's contents; a pattern without a slash
/// matches a basename anywhere; a leading slash anchors to the root; `*` and
/// `**` as in globs.
pub fn codeowners_matches(pattern: &str, path: &str) -> bool {
    super::pattern::matches(pattern, path)
}
